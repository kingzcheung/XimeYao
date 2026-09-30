use interprocess::os::windows::named_pipe::{pipe_mode::Bytes, DuplexPipeStream};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const MAX_RESPONSE_SIZE: usize = 1024 * 1024;
const READ_TIMEOUT_MS: u64 = 100;

#[derive(Debug)]
pub enum IpcError {
    ConnectionFailed(String),
    SerializeFailed(String),
    DeserializeFailed(String),
    WriteFailed(String),
    ReadFailed(String),
    EmptyResponse,
    ResponseTooLarge,
    Timeout,
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::ConnectionFailed(s) => write!(f, "ConnectionFailed: {}", s),
            IpcError::SerializeFailed(s) => write!(f, "SerializeFailed: {}", s),
            IpcError::DeserializeFailed(s) => write!(f, "DeserializeFailed: {}", s),
            IpcError::WriteFailed(s) => write!(f, "WriteFailed: {}", s),
            IpcError::ReadFailed(s) => write!(f, "ReadFailed: {}", s),
            IpcError::EmptyResponse => write!(f, "EmptyResponse"),
            IpcError::ResponseTooLarge => {
                write!(f, "ResponseTooLarge (max {} bytes)", MAX_RESPONSE_SIZE)
            }
            IpcError::Timeout => write!(f, "Timeout ({}ms)", READ_TIMEOUT_MS),
        }
    }
}

pub fn check_server_running() -> bool {
    let pipe_path = crate::messages::get_pipe_path();
    match DuplexPipeStream::<Bytes>::connect_by_path(pipe_path) {
        Ok(_) => true,
        Err(_) => false,
    }
}

pub fn stop_server() -> bool {
    if check_server_running() {
        IpcClient::shutdown_server()
    } else {
        false
    }
}

pub struct IpcClient {
    pipe: DuplexPipeStream<Bytes>,
}

impl IpcClient {
    pub fn connect() -> Result<Self, IpcError> {
        let pipe_path = crate::messages::get_pipe_path();

        match DuplexPipeStream::connect_by_path(pipe_path) {
            Ok(pipe) => Ok(Self { pipe }),
            Err(e) => Err(IpcError::ConnectionFailed(format!("{:?}", e))),
        }
    }

    pub fn send_request(
        &mut self,
        request: &crate::IpcRequest,
    ) -> Result<crate::IpcResponse, IpcError> {
        let json = serde_json::to_vec(request)
            .map_err(|e| IpcError::SerializeFailed(format!("{:?}", e)))?;

        self.pipe
            .write_all(&json)
            .map_err(|e| IpcError::WriteFailed(format!("write_all json: {:?}", e)))?;
        self.pipe
            .write_all(&[0])
            .map_err(|e| IpcError::WriteFailed(format!("write_all terminator: {:?}", e)))?;
        self.pipe
            .flush()
            .map_err(|e| IpcError::WriteFailed(format!("flush: {:?}", e)))?;

        let mut response_buf = Vec::new();
        let mut byte = [0u8; 1];
        let start_time = Instant::now();

        loop {
            if response_buf.len() > MAX_RESPONSE_SIZE {
                return Err(IpcError::ResponseTooLarge);
            }
            if start_time.elapsed() > Duration::from_millis(READ_TIMEOUT_MS) {
                return Err(IpcError::Timeout);
            }
            match self.pipe.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == 0 {
                        break;
                    }
                    response_buf.push(byte[0]);
                }
                Err(e) => return Err(IpcError::ReadFailed(format!("{:?}", e))),
            }
        }

        if response_buf.is_empty() {
            return Err(IpcError::EmptyResponse);
        }

        serde_json::from_slice(&response_buf)
            .map_err(|e| IpcError::DeserializeFailed(format!("{:?}", e)))
    }

    pub fn send_oneway(&mut self, request: &crate::IpcRequest) -> Result<(), IpcError> {
        let json = serde_json::to_vec(request)
            .map_err(|e| IpcError::SerializeFailed(format!("{:?}", e)))?;
        self.pipe
            .write_all(&json)
            .map_err(|e| IpcError::WriteFailed(format!("{:?}", e)))?;
        self.pipe
            .write_all(&[0])
            .map_err(|e| IpcError::WriteFailed(format!("{:?}", e)))?;
        self.pipe
            .flush()
            .map_err(|e| IpcError::WriteFailed(format!("{:?}", e)))?;

        let mut response_buf = Vec::new();
        let mut byte = [0u8; 1];
        let start_time = Instant::now();

        loop {
            if response_buf.len() > MAX_RESPONSE_SIZE {
                return Err(IpcError::ResponseTooLarge);
            }
            if start_time.elapsed() > Duration::from_millis(READ_TIMEOUT_MS) {
                return Err(IpcError::Timeout);
            }
            match self.pipe.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == 0 {
                        break;
                    }
                    response_buf.push(byte[0]);
                }
                Err(e) => return Err(IpcError::ReadFailed(format!("{:?}", e))),
            }
        }
        Ok(())
    }

    pub fn shutdown_server() -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ShutdownServer,
                session_id: 0,
                data: crate::IpcRequestData::None,
            };
            client.send_oneway(&request).is_ok()
        } else {
            false
        }
    }

    pub fn reload_config() -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ReloadConfig,
                session_id: 0,
                data: crate::IpcRequestData::None,
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub fn reload_plugins() -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ReloadPlugins,
                session_id: 0,
                data: crate::IpcRequestData::None,
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// rime 用户资料同步（同步执行，词典大时可能需要数秒）。
    pub fn sync_user_data() -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::SyncUserData,
                session_id: 0,
                data: crate::IpcRequestData::None,
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// 列出用户词典（含快照目录）；服务未运行返回 None。
    pub fn list_user_dicts() -> Option<crate::DictResponse> {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ListUserDicts,
                session_id: 0,
                data: crate::IpcRequestData::None,
            };
            match client.send_request(&request) {
                Ok(response) => response.dict_response,
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// 备份用户词典快照到同步目录。
    pub fn backup_user_dict(dict: &str) -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::BackupUserDict,
                session_id: 0,
                data: crate::IpcRequestData::UserDict(dict.to_string()),
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// 从快照文件恢复用户词典。
    pub fn restore_user_dict(snapshot_path: &str) -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::RestoreUserDict,
                session_id: 0,
                data: crate::IpcRequestData::UserDictPath(snapshot_path.to_string()),
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// 导出用户词典为文本，返回记录条数。
    pub fn export_user_dict(dict: &str, path: &str) -> Option<i32> {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ExportUserDict,
                session_id: 0,
                data: crate::IpcRequestData::UserDictFile(dict.to_string(), path.to_string()),
            };
            match client.send_request(&request) {
                Ok(response) => response.dict_response.map(|d| d.count),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// 从文本导入用户词典，返回记录条数。
    pub fn import_user_dict(dict: &str, path: &str) -> Option<i32> {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ImportUserDict,
                session_id: 0,
                data: crate::IpcRequestData::UserDictFile(dict.to_string(), path.to_string()),
            };
            match client.send_request(&request) {
                Ok(response) => response.dict_response.map(|d| d.count),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// 请求 server 弹出系统通知（server 有 MSIX 包身份，toast 可归属）。
    pub fn show_toast(title: &str, body: &str) -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::ShowToast,
                session_id: 0,
                data: crate::IpcRequestData::Toast(crate::ToastMessage {
                    title: title.to_string(),
                    body: body.to_string(),
                }),
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub fn select_schema(schema_id: &str) -> bool {
        if let Ok(mut client) = Self::connect() {
            let request = crate::IpcRequest {
                command: crate::IpcCommand::SelectSchema,
                session_id: 0,
                data: crate::IpcRequestData::SelectSchema(schema_id.to_string()),
            };
            match client.send_request(&request) {
                Ok(response) => response.success,
                Err(_) => false,
            }
        } else {
            false
        }
    }
}
