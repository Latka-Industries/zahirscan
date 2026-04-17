//! Runtime checks for NetCDF tooling (complements the `netcdf` crate’s link-time `libnetcdf`).

use std::process::Command;

use anyhow::Result;

/// Check that NetCDF development tooling is visible (similar to [`crate::utils::ffprobe_handler::check_ffprobe_available`]).
///
/// Uses `nc-config --version`, then `pkg-config --modversion netcdf`, so builds that only have
/// a linker-visible `libnetcdf` without those helpers may still open files at runtime.
///
/// # Errors
///
/// Returns [`anyhow::Error`] if neither probe succeeds.
pub fn check_netcdf_available() -> Result<()> {
    if Command::new("nc-config")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Ok(());
    }
    if Command::new("pkg-config")
        .args(["--modversion", "netcdf"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Ok(());
    }
    log::warn!(
        "NetCDF helpers not found (`nc-config` / `pkg-config netcdf`). Skipping NetCDF metadata.\n\
         Install NetCDF (e.g. `brew install netcdf` on macOS, `libnetcdf-dev` on Debian/Ubuntu) \
         so `libnetcdf` and these tools are available:\n\
         https://docs.unidata.ucar.edu/netcdf/"
    );
    Err(anyhow::anyhow!(
        "NetCDF (nc-config / pkg-config) not available"
    ))
}
