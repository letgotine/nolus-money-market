use sdk::cosmwasm_schema::{export_schema, schema_for};
use timealarms::msg::{ExecuteMsg, InstantiateMsg};

fn main() {
    let out_dir = schema::prep_out_dir().expect("The output directory should be valid");

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
}
