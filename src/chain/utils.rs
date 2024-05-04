use utxorpc::spec::sync::BlockRef;

pub fn block_ref(index: u64, hash: String) -> BlockRef {
    BlockRef {
        index: index,
        hash: hash.into(),
    }
}
