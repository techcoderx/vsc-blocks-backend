use walrus::{ Module, ExportItem };

pub fn list_exports(bytecode: &Vec<u8>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
  // Parse the WASM module
  let module = Module::from_buffer(bytecode.as_slice())?;
  let mut result: Vec<String> = Vec::new();

  // List all exports
  for export in module.exports.iter() {
    match export.item {
      ExportItem::Function(_) => {
        if &export.name != "_initialize" && &export.name != "alloc" {
          result.push(export.name.clone());
        }
      }
      _ => (),
    }
  }

  Ok(result)
}

#[cfg(test)]
mod tests {
  use std::fs;
  use super::list_exports;

  #[test]
  fn test_hello_world() {
    let file = fs::read("../ipfs_dag/test/hello-world.wasm").unwrap();
    let exports = list_exports(&file).expect("should list wasm exports");
    assert_eq!(exports.contains(&String::from("entrypoint")), true);
    assert_eq!(exports.contains(&String::from("hello_world")), true);
  }
}
