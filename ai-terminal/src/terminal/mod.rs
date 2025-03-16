pub mod commands;
pub mod utils;
pub mod autocomplete;

// Re-export specific items from the submodules
pub use utils::detect_os_info;
pub use utils::extract_commands;

// Re-export everything from the submodules
pub use commands::*;
pub use utils::*;
pub use autocomplete::*; 