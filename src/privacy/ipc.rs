//! Current-user-only secret transport. No terminal engine or renderer state is shared.
#[cfg(windows)]
use super::wire::{MAX_FIELDS, MAX_MESSAGE, MAX_VALUE};
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
};
pub struct Report {
    pub tab: u64,
    pub fields: Vec<(String, String)>,
}
pub struct Broker {
    pub path: String,
    capabilities: Arc<Mutex<HashMap<u64, String>>>,
    receiver: mpsc::Receiver<Report>,
    #[cfg(windows)]
    stop: super::wire::windows_pipe::Handle,
    join: Option<JoinHandle<()>>,
}
impl Broker {
    pub fn register(
        &self,
        tab: u64,
    ) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, String> {
        let token = capability()?;
        self.capabilities
            .lock()
            .map_err(|_| "Privacy registry unavailable")?
            .insert(tab, token.clone());
        Ok(vec![
            ("JUNA_PRIVACY_PIPE".into(), self.path.clone().into()),
            ("JUNA_PRIVACY_TOKEN".into(), token.into()),
            (
                "JUNA_PRIVACY_SERVER".into(),
                std::process::id().to_string().into(),
            ),
        ])
    }
    pub fn unregister(&self, tab: u64) {
        if let Ok(mut caps) = self.capabilities.lock() {
            caps.remove(&tab);
        }
    }
    pub fn poll(&self) -> Option<Report> {
        self.receiver.try_recv().ok()
    }
    #[cfg(not(windows))]
    pub fn new() -> Result<Self, String> {
        Err("Protected Junafetch IPC requires Windows".into())
    }
    #[cfg(windows)]
    pub fn new() -> Result<Self, String> {
        use super::wire::windows_pipe::Handle;
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::INVALID_HANDLE_VALUE,
                Security::SECURITY_ATTRIBUTES,
                Storage::FileSystem::{
                    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
                },
                System::{
                    Pipes::{
                        CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
                        PIPE_TYPE_BYTE, PIPE_WAIT,
                    },
                    Threading::CreateEventW,
                },
            },
        };
        let path = format!(r"\\.\pipe\JunaPrivacy-{}", capability()?);
        let name = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let descriptor = security_descriptor()?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        // SAFETY: name and current-user security descriptor live through creation; remote access and handle inheritance are disabled.
        let raw = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_MESSAGE as u32,
                MAX_MESSAGE as u32,
                3000,
                Some(&attributes),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            return Err("Cannot create private local pipe".into());
        }
        let handle = Handle(raw);
        // SAFETY: private unnamed event is retained until the worker is signalled and joined.
        let stop = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
            .map(Handle)
            .map_err(|_| "Cannot create privacy stop event")?;
        let pipe_value = raw.0 as usize;
        let stop_value = stop.0 .0 as usize;
        let capabilities = Arc::new(Mutex::new(HashMap::new()));
        let caps = capabilities.clone();
        let (sender, receiver) = mpsc::sync_channel(8);
        let join = std::thread::Builder::new()
            .name("juna-privacy".into())
            .spawn(move || server(pipe_value, stop_value, caps, sender))
            .map_err(|_| "Cannot start privacy worker")?;
        // Ownership of the pipe handle transfers to the worker only after successful spawn.
        std::mem::forget(handle);
        Ok(Self {
            path,
            capabilities,
            receiver,
            stop,
            join: Some(join),
        })
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: stop is a live event owned by this broker; join completes before it is closed.
        unsafe {
            let _ = windows::Win32::System::Threading::SetEvent(self.stop.0);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
#[cfg(windows)]
fn capability() -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let mut bytes = [0u8; 32];
    // SAFETY: system RNG writes exactly the owned output slice; no algorithm handle is required with this flag.
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG).ok() }
        .map_err(|_| "Privacy randomness unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[cfg(not(windows))]
fn capability() -> Result<String, String> {
    Err("Privacy randomness requires Windows".into())
}
#[cfg(windows)]
struct Local(*mut std::ffi::c_void);
#[cfg(windows)]
impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: uniquely owns memory returned by the security conversion APIs using LocalAlloc.
        unsafe {
            let _ = windows::Win32::Foundation::LocalFree(Some(
                windows::Win32::Foundation::HLOCAL(self.0),
            ));
        }
    }
}
#[cfg(windows)]
fn security_descriptor() -> Result<Local, String> {
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::{
            Foundation::HANDLE,
            Security::{
                Authorization::{
                    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                    SDDL_REVISION_1,
                },
                GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };
    let mut token = HANDLE::default();
    // SAFETY: requests read-only identity information for this process; never impersonates or changes privileges.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|_| "Privacy user identity unavailable")?;
    let token = super::wire::windows_pipe::Handle(token);
    let mut length = 0;
    // SAFETY: the zero-length call queries required storage and writes only length.
    unsafe {
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut length);
    }
    if length == 0 || length > 65536 {
        return Err("Invalid privacy user identity".into());
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: usize storage provides TOKEN_USER alignment and at least the reported byte length; SID remains live during conversion.
    let sid = unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            length,
            &mut length,
        )
        .map_err(|_| "Privacy user identity unavailable")?;
        let user = &*storage.as_ptr().cast::<TOKEN_USER>();
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(|_| "Privacy SID unavailable")?;
        let allocation = Local(text.0.cast());
        let text = text.to_string().map_err(|_| "Invalid privacy SID")?;
        drop(allocation);
        text
    };
    let sddl = format!("D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: owned terminated SDDL grants only this user access; returned LocalAlloc storage is guarded.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(|_| "Cannot create privacy ACL")?;
    Ok(Local(descriptor.0))
}
#[cfg(windows)]
fn server(
    pipe_value: usize,
    stop_value: usize,
    capabilities: Arc<Mutex<HashMap<u64, String>>>,
    sender: mpsc::SyncSender<Report>,
) {
    use super::wire::windows_pipe::{Handle, Pipe};
    use windows::Win32::{
        Foundation::{ERROR_PIPE_CONNECTED, HANDLE, WAIT_OBJECT_0},
        System::{
            Pipes::{ConnectNamedPipe, DisconnectNamedPipe},
            Threading::WaitForSingleObject,
        },
    };
    let handle = Handle(HANDLE(pipe_value as *mut std::ffi::c_void));
    let stop = HANDLE(stop_value as *mut std::ffi::c_void);
    let Ok(pipe) = Pipe::new(handle) else { return };
    loop {
        // SAFETY: borrowed stop event remains open until this worker joins; no pointer payload.
        if unsafe { WaitForSingleObject(stop, 0) } == WAIT_OBJECT_0 {
            break;
        }
        let mut operation = pipe.operation();
        // SAFETY: live pipe and OVERLAPPED remain owned through completion/cancellation below.
        let started = unsafe { ConnectNamedPipe(pipe.handle.0, Some(&mut operation)) };
        let connected = matches!(&started, Err(error) if error.code() == ERROR_PIPE_CONNECTED.to_hresult())
            || pipe
                .finish(&mut operation, started, Some(stop), u32::MAX)
                .is_ok();
        if !connected {
            break;
        }
        let _ = receive(&pipe, stop, &capabilities, &sender);
        // SAFETY: all connection I/O is joined before disconnect; values never echo back to clients.
        unsafe {
            let _ = DisconnectNamedPipe(pipe.handle.0);
        }
    }
}
#[cfg(windows)]
fn receive(
    pipe: &super::wire::windows_pipe::Pipe,
    stop: windows::Win32::Foundation::HANDLE,
    capabilities: &Mutex<HashMap<u64, String>>,
    sender: &mpsc::SyncSender<Report>,
) -> Result<(), String> {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut prefix = [0; 4];
    pipe.read_exact(&mut prefix, Some(stop), deadline)?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length == 0 || length > MAX_MESSAGE {
        return Err("Invalid privacy message size".into());
    }
    let mut bytes = vec![0; length];
    pipe.read_exact(&mut bytes, Some(stop), deadline)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid privacy message")?;
    let token = value["token"]
        .as_str()
        .ok_or("Missing privacy capability")?;
    let tab = capabilities
        .lock()
        .map_err(|_| "Privacy registry unavailable")?
        .iter()
        .find_map(|(tab, cap)| constant_equal(token.as_bytes(), cap.as_bytes()).then_some(*tab))
        .ok_or("Privacy capability rejected")?;
    let values = value["fields"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= MAX_FIELDS)
        .ok_or("Invalid protected fields")?;
    let mut fields = Vec::new();
    let mut ids = Vec::new();
    for value in values {
        let text = value
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= MAX_VALUE && !s.chars().any(char::is_control))
            .ok_or("Invalid protected value")?;
        let id = capability()?[..32].to_owned();
        fields.push((id.clone(), text.to_owned()));
        ids.push(id);
    }
    sender
        .try_send(Report { tab, fields })
        .map_err(|_| "Privacy receiver busy")?;
    let response = serde_json::to_vec(&ids).map_err(|_| "Cannot encode privacy acknowledgement")?;
    pipe.write_all(&(response.len() as u32).to_le_bytes(), Some(stop), deadline)?;
    pipe.write_all(&response, Some(stop), deadline)?;
    let mut acknowledged = [0];
    pipe.read_exact(&mut acknowledged, Some(stop), deadline)?;
    if acknowledged != [1] {
        return Err("Invalid privacy acknowledgement".into());
    }
    Ok(())
}
#[cfg(any(windows, test))]
fn constant_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != 64 || b.len() != 64 {
        return false;
    }
    a.iter().zip(b).fold(0u8, |bits, (a, b)| bits | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn capabilities_are_fixed_length_and_compared_completely() {
        assert!(super::constant_equal(&[b'a'; 64], &[b'a'; 64]));
        assert!(!super::constant_equal(&[b'a'; 63], &[b'a'; 64]));
        let mut changed = [b'a'; 64];
        changed[63] = b'b';
        assert!(!super::constant_equal(&[b'a'; 64], &changed));
    }
    #[cfg(windows)]
    #[test]
    fn current_user_pipe_authenticates_values_and_cancels_blocked_clients() {
        use super::*;
        use crate::privacy::wire::windows_pipe::client;
        use std::time::{Duration, Instant};
        let broker = Broker::new().unwrap();
        let environment = broker.register(7).unwrap();
        let token = environment
            .iter()
            .find(|(key, _)| key == "JUNA_PRIVACY_TOKEN")
            .unwrap()
            .1
            .to_string_lossy()
            .into_owned();
        let request = |cap: &str| -> Result<Vec<String>, String> {
            let pipe = client(&broker.path, std::process::id())?;
            let deadline = Instant::now() + Duration::from_secs(3);
            let data = serde_json::to_vec(
                &serde_json::json!({"token":cap,"fields":["192.0.2.55", "02:00:00:00:00:01"]}),
            )
            .unwrap();
            pipe.write_all(&(data.len() as u32).to_le_bytes(), None, deadline)?;
            pipe.write_all(&data, None, deadline)?;
            let mut prefix = [0; 4];
            pipe.read_exact(&mut prefix, None, deadline)?;
            let count = u32::from_le_bytes(prefix) as usize;
            if count > 4096 {
                return Err("Invalid response".into());
            }
            let mut bytes = vec![0; count];
            pipe.read_exact(&mut bytes, None, deadline)?;
            let ids = serde_json::from_slice(&bytes).map_err(|_| "Invalid response")?;
            pipe.write_all(&[1], None, deadline)?;
            Ok(ids)
        };
        assert!(request(&"0".repeat(64)).is_err());
        assert!(broker.poll().is_none());
        for _ in 0..4 {
            let ids = request(&token).unwrap();
            assert_eq!(ids.len(), 2);
            assert_ne!(ids[0], ids[1]);
            let deadline = Instant::now() + Duration::from_secs(1);
            let report = loop {
                if let Some(report) = broker.poll() {
                    break report;
                }
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            };
            assert_eq!(report.tab, 7);
            assert_eq!(report.fields[0], (ids[0].clone(), "192.0.2.55".into()));
        }
        broker.unregister(7);
        assert!(request(&token).is_err());
        assert!(broker.poll().is_none());
        assert!(client(&broker.path, std::process::id() + 1).is_err());
        let blocked = client(&broker.path, std::process::id()).unwrap();
        let start = Instant::now();
        drop(broker);
        assert!(start.elapsed() < Duration::from_secs(2));
        drop(blocked);
    }
}
