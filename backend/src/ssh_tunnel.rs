use crate::db::{DriverError, SshTunnelConfig};

pub struct SshTunnel {
    local_port: u16,
}

impl SshTunnel {
    pub async fn start(
        _ssh: &SshTunnelConfig,
        _remote_host: &str,
        _remote_port: u16,
    ) -> Result<Self, DriverError> {
        Err(DriverError::SshTunnelError(
            "SSH tunneling is disabled in the native rewrite until strict host-key verification is implemented".into(),
        ))
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}
