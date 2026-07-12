use crate::context::SharedInputContext;
use crate::schema_manager::SchemaManager;
use crate::ui::CandidateWindow;
use interprocess::os::windows::named_pipe::{pipe_mode::Bytes, PipeListenerOptions};
use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tracing::info;
use widestring::u16cstr;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
use winxime_ipc::{
    get_pipe_path, IpcCommand, IpcRequest, IpcRequestData, IpcResponse, SchemaMarketResponse,
};
use xime_config::XimeConfig;
use xime_rime::RimeEngine;

const MAX_BUFFER_SIZE: usize = 1024 * 1024;

pub fn run_ipc_server(
    engine: Arc<std::sync::Mutex<RimeEngine>>,
    context: Arc<SharedInputContext>,
    window: Arc<CandidateWindow>,
    ascii_mode: Arc<AtomicBool>,
    main_thread_id: u32,
    schema_mgr: Arc<SchemaManager>,
) {
    let pipe_path = get_pipe_path();
    tracing::info!("Winxime Server: creating named pipe at {}", pipe_path);

    let sd = SecurityDescriptor::deserialize(u16cstr!("D:(A;;GA;;;WD)"))
        .expect("Failed to create security descriptor");

    let listener = match PipeListenerOptions::new()
        .path(pipe_path)
        .mode(interprocess::os::windows::named_pipe::PipeMode::Bytes)
        .security_descriptor(Some(sd))
        .create_duplex::<Bytes>()
    {
        Ok(l) => l,
        Err(e) => {
            tracing::info!("Failed to create pipe listener: {}", e);
            return;
        }
    };

    tracing::info!("Waiting for client connections...");

    for pipe in listener.incoming() {
        match pipe {
            Ok(p) => {
                tracing::info!("Client connected!");
                let engine_clone = engine.clone();
                let context_clone = context.clone();
                let window_clone = window.clone();
                let ascii_mode_clone = ascii_mode.clone();
                let tid = main_thread_id;
                let schema_mgr_clone = schema_mgr.clone();
                std::thread::spawn(move || {
                    handle_connection(
                        p,
                        engine_clone,
                        context_clone,
                        window_clone,
                        ascii_mode_clone,
                        tid,
                        schema_mgr_clone,
                    );
                });
            }
            Err(e) => {
                tracing::info!("Failed to accept connection: {}", e);
            }
        }
    }
}

fn handle_connection(
    pipe: interprocess::os::windows::named_pipe::PipeStream<Bytes, Bytes>,
    engine: Arc<std::sync::Mutex<RimeEngine>>,
    context: Arc<SharedInputContext>,
    window: Arc<CandidateWindow>,
    ascii_mode: Arc<AtomicBool>,
    main_thread_id: u32,
    schema_mgr: Arc<SchemaManager>,
) {
    let (recv, send) = pipe.split();
    let mut reader = BufReader::new(recv);
    let mut writer = BufWriter::new(send);

    loop {
        let mut buffer = Vec::new();

        loop {
            if buffer.len() > MAX_BUFFER_SIZE {
                tracing::info!("Buffer too large, disconnecting client");
                return;
            }

            let mut byte = [0u8; 1];
            match reader.read(&mut byte) {
                Ok(0) => {
                    if buffer.is_empty() {
                        return;
                    }
                    break;
                }
                Ok(_) => {
                    if byte[0] == 0 {
                        break;
                    }
                    buffer.push(byte[0]);
                }
                Err(_) => return,
            }
        }

        if buffer.is_empty() {
            continue;
        }

        let request: IpcRequest = match serde_json::from_slice(&buffer) {
            Ok(r) => r,
            Err(_) => continue,
        };
        tracing::info!("Received request: {:?}", request.command);

        let response = process_request(
            &request,
            &engine,
            &context,
            &window,
            &ascii_mode,
            main_thread_id,
            &schema_mgr,
        );

        let json = match serde_json::to_vec(&response) {
            Ok(j) => j,
            Err(_) => continue,
        };
        if writer.write_all(&json).is_err() {
            break;
        }
        if writer.write_all(&[0]).is_err() {
            break;
        }
        if writer.flush().is_err() {
            break;
        }
    }
    tracing::info!("Client disconnected");
}

fn process_request(
    request: &IpcRequest,
    engine: &Arc<std::sync::Mutex<RimeEngine>>,
    context: &Arc<SharedInputContext>,
    window: &Arc<CandidateWindow>,
    ascii_mode: &Arc<AtomicBool>,
    main_thread_id: u32,
    schema_mgr: &Arc<SchemaManager>,
) -> IpcResponse {
    let mut eng = match engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => {
            tracing::info!("Engine lock would block, returning error response");
            return IpcResponse {
                success: false,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            };
        }
    };

    match request.command {
        IpcCommand::Echo => IpcResponse {
            success: true,
            session_id: request.session_id,
            context: None,
            status: None,
            schema_list: None,
        market_response: None,
        },

        IpcCommand::StartSession => {
            tracing::info!("StartSession");
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::EndSession => {
            tracing::info!("EndSession");
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::FocusIn => {
            tracing::info!("FocusIn");
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::FocusOut => {
            tracing::info!("FocusOut -> hide composition");
            eng.clear_composition();
            window.hide();
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::ProcessKeyEvent => {
            let is_ascii = ascii_mode.load(Ordering::Acquire);
            tracing::info!("Key event, ascii_mode={}", is_ascii);

            let suggestion_state = context.read(|c| c.suggestion_state.clone());

            if let Some(ref suggestion) = suggestion_state {
                tracing::info!(
                    "  -> in suggestion mode, suggestions: {:?}",
                    suggestion.suggestions
                );

                if let IpcRequestData::KeyEvent(key) = &request.data {
                    if key.keycode == 32 {
                        tracing::info!("  -> Space in suggestion mode, commit suggestion");
                        if suggestion.highlighted < suggestion.suggestions.len() {
                            let selected_word = &suggestion.suggestions[suggestion.highlighted];
                            tracing::info!("  -> committing suggestion word: {}", selected_word);

                            context.update(|ctx| {
                                ctx.suggestion_state = None;
                                ctx.commit_text = selected_word.clone();
                            });
                            window.hide();

                            return IpcResponse {
                                success: true,
                                session_id: request.session_id,
                                context: Some(winxime_ipc::Context {
                                    preedit: winxime_ipc::Text { str: String::new() },
                                    commit: Some(selected_word.clone()),
                                    candidates: winxime_ipc::CandidateInfo::default(),
                                }),
                                status: Some(get_ipc_status(&eng)),
                                schema_list: None,
                            market_response: None,
                            };
                        } else {
                            context.update(|ctx| {
                                ctx.suggestion_state = None;
                            });
                            window.hide();
                            return IpcResponse {
                                success: false,
                                session_id: request.session_id,
                                context: None,
                                status: Some(get_ipc_status(&eng)),
                                schema_list: None,
                            market_response: None,
                            };
                        }
                    } else if key.keycode >= 49 && key.keycode <= 57 {
                        let index = (key.keycode - 49) as usize;
                        tracing::info!("  -> Number key {} in suggestion mode", index + 1);
                        if index < suggestion.suggestions.len() {
                            let selected_word = &suggestion.suggestions[index];
                            tracing::info!("  -> committing suggestion word: {}", selected_word);

                            context.update(|ctx| {
                                ctx.suggestion_state = None;
                                ctx.commit_text = selected_word.clone();
                            });
                            window.hide();

                            return IpcResponse {
                                success: true,
                                session_id: request.session_id,
                                context: Some(winxime_ipc::Context {
                                    preedit: winxime_ipc::Text { str: String::new() },
                                    commit: Some(selected_word.clone()),
                                    candidates: winxime_ipc::CandidateInfo::default(),
                                }),
                                status: Some(get_ipc_status(&eng)),
                                schema_list: None,
                            market_response: None,
                            };
                        } else {
                            return IpcResponse {
                                success: false,
                                session_id: request.session_id,
                                context: None,
                                status: Some(get_ipc_status(&eng)),
                                schema_list: None,
                            market_response: None,
                            };
                        }
                    } else {
                        tracing::info!(
                            "  -> Other key in suggestion mode, clear suggestion and continue"
                        );
                        context.update(|ctx| {
                            ctx.suggestion_state = None;
                        });
                        window.hide();
                    }
                } else {
                    return IpcResponse {
                        success: false,
                        session_id: request.session_id,
                        context: None,
                        status: Some(get_ipc_status(&eng)),
                        schema_list: None,
                    market_response: None,
                    };
                }
            }

            if is_ascii {
                tracing::info!("  -> ASCII mode, not handling");
                return IpcResponse {
                    success: false,
                    session_id: request.session_id,
                    context: None,
                    status: Some(get_ipc_status(&eng)),
                    schema_list: None,
                market_response: None,
                };
            }

            let handled = match &request.data {
                IpcRequestData::KeyEvent(key) => {
                    tracing::info!("Key: {} mod: {}", key.keycode, key.modifiers);
                    let result = eng.process_key(key.keycode, key.modifiers);
                    tracing::info!("  handled: {}", result);
                    result
                }
                _ => false,
            };

            let commit = eng.get_commit();
            tracing::info!("  commit: {:?}", commit);
            info!("  input: {:?}", eng.get_input());
            info!("  composing: {}", eng.is_composing());

            if let Some(ref commit_text) = commit {
                tracing::info!(">>> COMMIT_TO_SCREEN: '{}'", commit_text);
            }

            let ipc_ctx = get_ipc_context(&eng, &commit);
            update_context(&mut eng, context, &commit);

            if let Some(ref commit_text) = commit {
                tracing::info!(">>> COMMIT_TO_SCREEN: '{}'", commit_text);

                tracing::info!("  -> hide (commit)");
                context.update(|ctx| {
                    ctx.suggestion_state = None;
                });
                window.hide();
                return IpcResponse {
                    success: handled,
                    session_id: request.session_id,
                    context: ipc_ctx,
                    status: Some(get_ipc_status(&eng)),
                    schema_list: None,
                market_response: None,
                };
            } else if !eng.is_composing() {
                tracing::info!("  -> hide (not composing)");
                context.update(|ctx| {
                    ctx.suggestion_state = None;
                });
                window.hide();
                return IpcResponse {
                    success: handled,
                    session_id: request.session_id,
                    context: ipc_ctx,
                    status: Some(get_ipc_status(&eng)),
                    schema_list: None,
                market_response: None,
                };
            } else if let Some(ctx) = &ipc_ctx {
                tracing::info!("  candies: {:?}", ctx.candidates.candies);
                if ctx.candidates.candies.is_empty() {
                    tracing::info!("  -> hide (no candidates)");
                    window.hide();
                } else {
                    let pos = context.read(|c| (c.caret_x, c.caret_y));
                    tracing::info!("  -> show at ({}, {})", pos.0, pos.1);
                    window.show(pos.0, pos.1);
                    info!("  -> update {} candies", ctx.candidates.candies.len());
                    window.update(ctx);
                }

                return IpcResponse {
                    success: handled,
                    session_id: request.session_id,
                    context: ipc_ctx,
                    status: Some(get_ipc_status(&eng)),
                    schema_list: None,
                market_response: None,
                };
            } else {
                return IpcResponse {
                    success: handled,
                    session_id: request.session_id,
                    context: ipc_ctx,
                    status: Some(get_ipc_status(&eng)),
                    schema_list: None,
                market_response: None,
                };
            }
        }

        IpcCommand::UpdateInputPosition => {
            match &request.data {
                IpcRequestData::Position(pos) => {
                    tracing::info!("Position: {},{}", pos.x, pos.y);
                    context.update(|ctx| {
                        ctx.caret_x = pos.x;
                        ctx.caret_y = pos.y;
                    });
                }
                _ => {}
            }

            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::ShutdownServer => {
            tracing::info!("Shutdown requested, posting WM_QUIT to main thread");
            unsafe {
                let _ = PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::ToggleAsciiMode => {
            tracing::info!("ToggleAsciiMode requested");
            let current = eng.is_ascii_mode();
            let new_mode = !current;
            tracing::info!("  -> current={}, setting to {}", current, new_mode);

            // Check if we were composing before the switch
            let was_composing = eng.is_composing();
            let input_text = if was_composing {
                eng.get_input().unwrap_or_default()
            } else {
                String::new()
            };

            // Clear composition in the engine
            if was_composing {
                tracing::info!("  -> clearing composition before switch");
                eng.clear_composition();
            }

            eng.set_option("ascii_mode", new_mode);
            ascii_mode.store(new_mode, Ordering::Release);
            crate::tray::update_tray_icon(new_mode);

            window.hide();

            // Build context response
            // When switching to ASCII mode with input, commit the input code
            // When switching to Chinese mode or no input, just clear the composition
            let ctx = if new_mode && !input_text.is_empty() {
                // Switching to ASCII mode: commit the input code
                tracing::info!(
                    "  -> commit_code: committing '{}' before switch to ASCII",
                    input_text
                );
                tracing::info!(">>> COMMIT_TO_SCREEN (toggle): '{}'", input_text);
                Some(winxime_ipc::Context {
                    preedit: winxime_ipc::Text { str: String::new() },
                    commit: Some(input_text),
                    candidates: winxime_ipc::CandidateInfo::default(),
                })
            } else if was_composing {
                // Was composing but not committing: indicate composition should be cleared
                tracing::info!("  -> clearing composition in TSF (no commit)");
                Some(winxime_ipc::Context {
                    preedit: winxime_ipc::Text { str: String::new() },
                    commit: None,
                    candidates: winxime_ipc::CandidateInfo::default(),
                })
            } else {
                None
            };

            update_context(&mut eng, &context, &None);

            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: ctx,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::ShowTrayIcon => {
            crate::tray::show_icon();
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::HideTrayIcon => {
            crate::tray::hide_icon();
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::HideCandidates => {
            context.update(|ctx| {
                ctx.is_composing = false;
                ctx.composition.preedit.clear();
                ctx.candidates.clear();
                ctx.commit_text.clear();
            });
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::ReloadConfig => {
            tracing::info!("ReloadConfig requested");
            let deploy_result = eng.redeploy();
            tracing::info!("  redeploy result: {}", deploy_result);
            IpcResponse {
                success: deploy_result,
                session_id: request.session_id,
                context: None,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::GetSchemaList => {
            tracing::info!("GetSchemaList requested");
            let schemas = eng.get_schema_list();
            let schema_list = schemas
                .iter()
                .map(|(id, name)| winxime_ipc::SchemaInfo {
                    schema_id: id.clone(),
                    schema_name: name.clone(),
                })
                .collect();
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: Some(get_ipc_status(&eng)),
                schema_list: Some(schema_list),
                market_response: None,
            }
        }

        IpcCommand::SelectSchema => {
            tracing::info!("SelectSchema requested");
            let schema_id = match &request.data {
                winxime_ipc::IpcRequestData::SelectSchema(id) => Some(id.clone()),
                _ => None,
            };

            match schema_id {
                Some(id) => {
                    tracing::info!("  -> selecting schema: {}", id);
                    if eng.select_schema(&id) {
                        tracing::info!("  -> schema selected successfully");
                        IpcResponse {
                            success: true,
                            session_id: request.session_id,
                            context: None,
                            status: Some(get_ipc_status(&eng)),
                            schema_list: None,
                        market_response: None,
                        }
                    } else {
                        tracing::info!("  -> schema selection failed");
                        IpcResponse {
                            success: false,
                            session_id: request.session_id,
                            context: None,
                            status: Some(get_ipc_status(&eng)),
                            schema_list: None,
                        market_response: None,
                        }
                    }
                }
                None => IpcResponse {
                    success: false,
                    session_id: request.session_id,
                    context: None,
                    status: Some(get_ipc_status(&eng)),
                    schema_list: None,
                market_response: None,
                },
            }
        }

        IpcCommand::ShowRoot => {
            tracing::info!("ShowRoot requested");
            tracing::info!("  -> request.data type: {:?}", request.data);
            let letter = match &request.data {
                winxime_ipc::IpcRequestData::ShowRoot(c) => Some(*c),
                _ => None,
            };

            tracing::info!("  -> letter: {:?}", letter);

            match letter {
                Some(c) => {
                    let config = XimeConfig::load();
                    let schema_id = eng.get_status().map(|s| s.schema_id).unwrap_or_default();
                    tracing::info!(
                        "  -> config loaded, schema_id={}, checking root for '{}'",
                        schema_id,
                        c
                    );
                    let root = config.get_root_for_key(&schema_id, c);
                    tracing::info!("  -> root result: {:?}", root);
                    if let Some(root) = root {
                        tracing::info!("  -> showing root for '{}': {}", c, root);
                        let result = window.show_root(c, &root);
                        tracing::info!("  -> show_root result: {:?}", result);
                        IpcResponse {
                            success: result.is_ok(),
                            session_id: request.session_id,
                            context: None,
                            status: Some(get_ipc_status(&eng)),
                            schema_list: None,
                        market_response: None,
                        }
                    } else {
                        tracing::warn!("  -> no root for key '{}' in schema '{}'", c, schema_id);
                        IpcResponse {
                            success: false,
                            session_id: request.session_id,
                            context: None,
                            status: Some(get_ipc_status(&eng)),
                            schema_list: None,
                        market_response: None,
                        }
                    }
                }
                None => {
                    tracing::info!("  -> no letter provided");
                    IpcResponse {
                        success: false,
                        session_id: request.session_id,
                        context: None,
                        status: Some(get_ipc_status(&eng)),
                        schema_list: None,
                    market_response: None,
                    }
                }
            }
        }

        IpcCommand::HideRoot => {
            tracing::info!("HideRoot requested");
            window.hide_root();

            let ipc_ctx = get_ipc_context(&eng, &None);
            if let Some(ctx) = &ipc_ctx {
                if !ctx.candidates.candies.is_empty() {
                    let pos = context.read(|c| (c.caret_x, c.caret_y));
                    window.show(pos.0, pos.1);
                    window.update(ctx);
                }
            }

            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::SelectCandidate => {
            tracing::info!("SelectCandidate requested");
            let index = match &request.data {
                winxime_ipc::IpcRequestData::SelectIndex(i) => *i,
                _ => 0,
            };

            tracing::info!("  -> selecting candidate at index {}", index);
            let selected = eng.select_candidate(index);
            tracing::info!("  -> select result: {}", selected);

            let commit = eng.get_commit();
            tracing::info!("  -> commit: {:?}", commit);

            let ipc_ctx = get_ipc_context(&eng, &commit);
            update_context(&mut eng, context, &commit);

            if commit.is_some() {
                tracing::info!("  -> hide (commit after select)");
                window.hide();
            } else if !eng.is_composing() {
                tracing::info!("  -> hide (not composing after select)");
                window.hide();
            } else if let Some(ctx) = &ipc_ctx {
                let pos = context.read(|c| (c.caret_x, c.caret_y));
                window.show(pos.0, pos.1);
                window.update(ctx);
            }

            IpcResponse {
                success: selected,
                session_id: request.session_id,
                context: ipc_ctx,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::ChangePage => {
            tracing::info!("ChangePage requested");
            let backward = match &request.data {
                winxime_ipc::IpcRequestData::ChangePage(b) => *b,
                _ => false,
            };

            tracing::info!("  -> backward: {}", backward);
            let changed = eng.change_page(backward);
            tracing::info!("  -> change result: {}", changed);

            let ipc_ctx = get_ipc_context(&eng, &None);
            if let Some(ctx) = &ipc_ctx {
                let pos = context.read(|c| (c.caret_x, c.caret_y));
                window.show(pos.0, pos.1);
                window.update(ctx);
            }

            IpcResponse {
                success: changed,
                session_id: request.session_id,
                context: ipc_ctx,
                status: Some(get_ipc_status(&eng)),
                schema_list: None,
            market_response: None,
            }
        }

        IpcCommand::FetchSchemaIndex => {
            tracing::info!("FetchSchemaIndex requested");
            match schema_mgr.fetch_index() {
                Ok(text) => IpcResponse {
                    success: true,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::Index(text)),
                },
                Err(e) => IpcResponse {
                    success: false,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::Error(e)),
                },
            }
        }

        IpcCommand::DownloadSchema => {
            tracing::info!("DownloadSchema requested");
            let dl = match &request.data {
                winxime_ipc::IpcRequestData::SchemaDownload(d) => d,
                _ => {
                    return IpcResponse {
                        success: false,
                        session_id: request.session_id,
                        context: None,
                        status: None,
                        schema_list: None,
                        market_response: Some(SchemaMarketResponse::Error(
                            "无效的请求数据".to_string(),
                        )),
                    }
                }
            };
            let result = schema_mgr.download_schema(
                &dl.schema_id,
                &dl.url,
                dl.sha256.as_deref(),
                &dl.filename,
            );
            match result {
                Ok(()) => IpcResponse {
                    success: true,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::DownloadDone(
                        dl.schema_id.clone(),
                    )),
                },
                Err(e) => IpcResponse {
                    success: false,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::Error(e)),
                },
            }
        }

        IpcCommand::InstallSchema => {
            tracing::info!("InstallSchema requested");
            let sid = match &request.data {
                winxime_ipc::IpcRequestData::SchemaInstall(d) => &d.schema_id,
                _ => {
                    return IpcResponse {
                        success: false,
                        session_id: request.session_id,
                        context: None,
                        status: None,
                        schema_list: None,
                        market_response: Some(SchemaMarketResponse::Error(
                            "无效的请求数据".to_string(),
                        )),
                    }
                }
            };
            match schema_mgr.install_schema(sid) {
                Ok(()) => IpcResponse {
                    success: true,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::InstallDone(sid.clone())),
                },
                Err(e) => IpcResponse {
                    success: false,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::Error(e)),
                },
            }
        }

        IpcCommand::UninstallSchema => {
            tracing::info!("UninstallSchema requested");
            let sid = match &request.data {
                winxime_ipc::IpcRequestData::SchemaUninstall(d) => &d.schema_id,
                _ => {
                    return IpcResponse {
                        success: false,
                        session_id: request.session_id,
                        context: None,
                        status: None,
                        schema_list: None,
                        market_response: Some(SchemaMarketResponse::Error(
                            "无效的请求数据".to_string(),
                        )),
                    }
                }
            };
            match schema_mgr.uninstall_schema(sid) {
                Ok(()) => IpcResponse {
                    success: true,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::UninstallDone(sid.clone())),
                },
                Err(e) => IpcResponse {
                    success: false,
                    session_id: request.session_id,
                    context: None,
                    status: None,
                    schema_list: None,
                    market_response: Some(SchemaMarketResponse::Error(e)),
                },
            }
        }

        IpcCommand::ListMarketSchemas => {
            tracing::info!("ListMarketSchemas requested");
            let packages = schema_mgr.list_market_schemas();
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
                market_response: Some(SchemaMarketResponse::PackageList(packages)),
            }
        }

        IpcCommand::ListInstalledPackages => {
            tracing::info!("ListInstalledPackages requested");
            let packages = schema_mgr.list_installed_packages();
            IpcResponse {
                success: true,
                session_id: request.session_id,
                context: None,
                status: None,
                schema_list: None,
                market_response: Some(SchemaMarketResponse::InstalledList(packages)),
            }
        }

        _ => IpcResponse {
            success: false,
            session_id: request.session_id,
            context: None,
            status: None,
            schema_list: None,
            market_response: None,
        },
    }
}

fn update_context(
    eng: &mut RimeEngine,
    context: &Arc<SharedInputContext>,
    commit: &Option<String>,
) {
    use crate::context::CandidateInfo;
    context.update(|ctx| {
        ctx.is_composing = eng.is_composing();
        ctx.composition.preedit = eng.get_input().unwrap_or_default();
        ctx.commit_text = commit.clone().unwrap_or_default();

        let cand_list = eng.get_candidates();
        ctx.candidates = cand_list
            .candidates
            .iter()
            .map(|c| CandidateInfo {
                text: c.text.clone(),
                comment: c.comment.clone().unwrap_or_default(),
            })
            .collect();
    });
}

fn get_ipc_status(eng: &RimeEngine) -> winxime_ipc::Status {
    let status = eng.get_status();
    winxime_ipc::Status {
        composing: eng.is_composing(),
        ascii_mode: status.as_ref().map(|s| s.is_ascii_mode).unwrap_or(false),
        schema_id: status
            .as_ref()
            .map(|s| s.schema_id.clone())
            .unwrap_or_default(),
        schema_name: status
            .as_ref()
            .map(|s| s.schema_name.clone())
            .unwrap_or_default(),
    }
}

fn get_ipc_context(eng: &RimeEngine, commit: &Option<String>) -> Option<winxime_ipc::Context> {
    let composing = eng.is_composing();

    if !composing && commit.is_none() {
        return None;
    }

    let cand_list = eng.get_candidates();

    Some(winxime_ipc::Context {
        preedit: winxime_ipc::Text {
            str: eng.get_input().unwrap_or_default(),
        },
        commit: commit.clone(),
        candidates: winxime_ipc::CandidateInfo {
            current_page: cand_list.page_no as u32,
            total_pages: (if cand_list.is_last_page {
                cand_list.page_no + 1
            } else {
                cand_list.page_no + 2
            }) as u32,
            highlighted: cand_list.highlighted,
            is_last_page: cand_list.is_last_page,
            candies: cand_list
                .candidates
                .iter()
                .map(|c| winxime_ipc::Text {
                    str: c.text.clone(),
                })
                .collect(),
            comments: cand_list
                .candidates
                .iter()
                .map(|c| winxime_ipc::Text {
                    str: c.comment.clone().unwrap_or_default(),
                })
                .collect(),
            labels: Vec::new(),
        },
    })
}


