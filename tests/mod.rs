mod integration;
mod unit_tests;

use zahirscan::RuntimeConfig;

pub(crate) fn get_test_config() -> RuntimeConfig {
    RuntimeConfig::default()
}
