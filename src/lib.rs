use unity_capture::UnityCapture;

pub mod obs_vcam;
pub mod unity_capture;

pub enum Camera {
    UnityCapture(UnityCapture),
    // ObsVcam(obs_vcam::ObsVcam)
}

pub enum Error {
    UnityCaptureNotFound,
    UnityCaptureNotRunning,
    UnityCaptureAlreadyRunning,
    UnityCaptureNotInitialized,
    UnityCaptureUnknownError,
    SendresToolarge,
    SendresWarnFrameskip,
}
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = match self {
            Error::UnityCaptureNotFound => "UnityCaptureNotFound",
            Error::UnityCaptureNotRunning => "UnityCaptureNotRunning",
            Error::UnityCaptureAlreadyRunning => "UnityCaptureAlreadyRunning",
            Error::UnityCaptureNotInitialized => "UnityCaptureNotInitialized",
            Error::UnityCaptureUnknownError => "UnityCaptureUnknownError",
            Error::SendresToolarge => "SendresToolarge",
            Error::SendresWarnFrameskip => "SendresWarnFrameskip",
        };
        write!(f, "{}", msg)
    }
}
impl Camera {
    pub fn new(width: i32, height: i32, device: &str) -> Result<Self, Error> {
        // TODO: Add windows support
        #[cfg(target_os = "windows")]
        let unity_capture = UnityCapture::new(width, height, device.to_owned())?;
        return Ok(Camera::UnityCapture(unity_capture));

        // TODO: Add macos support
        #[cfg(target_os = "macos")]
        Self
    }

    pub fn send(&mut self, data: Vec<u8>) -> Result<(), Error> {
        match self {
            Camera::UnityCapture(unity_capture) => unity_capture.send(data), // Camera::ObsVcam(obs_vcam) => obs_vcam.send(data),
        }
    }
}
