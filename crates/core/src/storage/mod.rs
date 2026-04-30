pub use self::trait_def::{
    ProjectConfig, Storage, StorageCredentials, StorageError, StorageTransaction, TestCase,
};
pub use self::local::{LocalStorage, LocalStorageInstance};

pub mod trait_def;
pub mod local;
