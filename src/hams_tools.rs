use hams::error::HamsError as InnerHamsError;
use hams::hams::Hams;
use hams::probe::AsyncHealthProbe;
use hams::probe::FFIProbe;
use hams::probe::manual::Manual as ProbeManual;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Debug, Error)]
pub enum HamsError {
    #[error("Failed to register HaMS shutdown closure: {0}")]
    RegisterShutdownClosure(#[source] InnerHamsError),
    #[error("Failed to start HaMS: {0}")]
    StartFailed(#[source] InnerHamsError),
}

pub struct HamsHarness {
    pub hams: Hams,
    pub ready_signal: ProbeManual,
}

impl HamsHarness {
    pub async fn init(mut hams: Hams, ct: CancellationToken) -> Result<Self, HamsError> {
        let ready_signal = ProbeManual::new("db-connected", false);

        hams.ready_insert_async(
            Box::new(FFIProbe::from(ready_signal.clone())) as Box<dyn AsyncHealthProbe>
        )
        .await;

        let ct_clone = ct.clone();
        hams.register_shutdown_closure(move || {
            info!("HaMS shutdown callback triggered, cancelling application token");
            ct_clone.cancel();
        })
        .map_err(HamsError::RegisterShutdownClosure)?;

        hams.start().map_err(HamsError::StartFailed)?;

        Ok(Self { hams, ready_signal })
    }
}

