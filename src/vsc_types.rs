use serde::{ Serialize, Deserialize };

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractById {
  pub error: Option<String>,
  pub contract_id: String,
  pub created_in_op: String,
  pub created_in_l1_block: usize,
  pub created_at: String,
  pub creator: String,
  pub name: String,
  pub description: String,
  pub code: String,
}
