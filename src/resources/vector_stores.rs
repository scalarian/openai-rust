/// Vector Stores API family placeholder.
#[derive(Clone, Debug, Default)]
pub struct VectorStores {
    /// Vector store files placeholder handle.
    pub files: VectorStoreFiles,
    /// Vector store file batches placeholder handle.
    pub file_batches: VectorStoreFileBatches,
}

/// Vector store files placeholder.
#[derive(Clone, Debug, Default)]
pub struct VectorStoreFiles;

/// Vector store file batches placeholder.
#[derive(Clone, Debug, Default)]
pub struct VectorStoreFileBatches;
