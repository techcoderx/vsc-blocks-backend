use libipld::{ block::Block, cbor::DagCborCodec, codec::Encode, multihash::Code::Sha2_256, raw::RawCodec, store::DefaultParams };

pub fn put_dag<T>(payload: &T) -> String where T: Encode<DagCborCodec> + ?Sized {
  let a = Block::<DefaultParams>::encode(DagCborCodec, Sha2_256, payload).unwrap();
  a.cid().to_string()
}

pub fn put_dag_raw<T>(payload: &T) -> String where T: Encode<RawCodec> + ?Sized {
  let a = Block::<DefaultParams>::encode(RawCodec, Sha2_256, payload).unwrap();
  a.cid().to_string()
}

#[cfg(test)]
mod tests {
  use std::fs;
  use super::{ put_dag, put_dag_raw };

  #[test]
  fn test_put_dag_string() {
    assert_eq!(put_dag("aaa").as_str(), "bafyreibajxuqjmh6az6vne5jqmmpgb6q6wpoffusbsmdfrokjoptkxk6dy");
  }

  #[test]
  fn test_put_dag_file() {
    let file = fs::read("test/build.wasm").unwrap();
    assert_eq!(put_dag(file.as_slice()).as_str(), "bafyreihmqkn5sql6zts7rpgf7iwuatpb4yj3tpw2gx7akpsucawy2hlbwm");
  }

  #[test]
  fn test_put_dag_file_raw() {
    // vsc1Bem8RnoLgGPP7E2MBN52ekrdVqy2LNpSqF
    let file = fs::read("test/hello-world.wasm").unwrap();
    assert_eq!(put_dag_raw(file.as_slice()).as_str(), "bafkreibwwj3fypek5uz6l3scacy47yw3b2adgbxmfab2ybmkarljaggbey");
  }
}
