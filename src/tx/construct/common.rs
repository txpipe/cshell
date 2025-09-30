

use std::{fs, path::PathBuf, str::FromStr};

use anyhow::{Result, Context};
use pallas::ledger::addresses::Address;
use serde_json::json;
use inquire::{Text, Confirm};

pub struct TransactionBuilder {
    pub ast: tx3_lang::ast::Program,
    pub def_index: usize,
}

impl TransactionBuilder {
    pub fn new(name: String, mut ast: tx3_lang::ast::Program) -> Result<Self> {
        let mut def_index = ast.txs.iter().position(|tx| tx.name.value == name);

        if def_index.is_none() {
            println!("Creating new transaction: {}", name);
            // Create it as JSON and parse it as TxDef
            // TODO: Make scope pub in tx3_lang and construct directly or implement `Default`
            let value = json!({
                "name": tx3_lang::ast::Identifier::new(name),
                "parameters": tx3_lang::ast::ParameterList {
                    parameters: Vec::new(),
                    span: tx3_lang::ast::Span::default(),
                },
                "references": [],
                "inputs": [],
                "outputs": [],
                "mints": [],
                "burns": [],
                "adhoc": [],
                "span": tx3_lang::ast::Span::default(),
                "collateral": [],
            });
            let tx_def: tx3_lang::ast::TxDef =
                serde_json::from_value(value)
                    .context("Failed to materialize TxDef from JSON template")?;

            ast.txs.push(tx_def);

            def_index = Some(ast.txs.len() - 1);
        }
        
        Ok(Self {
            ast: ast.clone(),
            def_index: def_index.unwrap(),
        })
    }

    pub fn from_ast(ast_path_buf: &PathBuf) -> Result<Self> {
        let ast = if ast_path_buf.exists() {
            let ast_content = fs::read_to_string(ast_path_buf)
                .context("Failed to read existing AST file")?;

            serde_json::from_str(&ast_content)
                .context("Failed to parse existing AST file")?
        } else {
            tx3_lang::ast::Program::default()
        };

        TransactionBuilder::new("new_transaction".to_string(), ast)
    }

    pub fn collect_inputs(&mut self, skip_question: bool) -> Result<()> {
        println!("\n📥 Transaction Inputs");
        println!("====================");

        if !skip_question {
            let add_inputs = Confirm::new("Do you want to add inputs to your transaction?")
                .with_default(true)
                .prompt()?;
    
            if !add_inputs {
                return Ok(());
            }
        }


        loop {
            let input_name = Text::new("Input name:")
                .with_help_message("Enter input name (or 'done' to finish)")
                .prompt()?;

            if input_name.eq_ignore_ascii_case("done") {
                break;
            }

            let mut input_block = tx3_lang::ast::InputBlock {
                name: input_name.clone(),
                span: tx3_lang::ast::Span::default(),
                many: false,
                fields: Vec::new(),
            };

            let utxo_ref = Text::new("Utxo Ref:")
                .with_help_message("Enter the Utxo for this input (txid#index)")
                .prompt()?;

            let parts: Vec<&str> = utxo_ref.split('#').collect();
            if parts.len() != 2 {
                println!("Invalid Utxo Ref format. Expected format: txid#index");
                continue;
            }

            // input_block.fields.push(tx3_lang::ast::InputBlockField::From(
            //     tx3_lang::ast::DataExpr::String(tx3_lang::ast::StringLiteral::new(address.to_bech32().unwrap())),
            // ));

            let txid = hex::decode(parts[0])
                .context("Invalid txid hex in UTxO reference")?;

            let index = parts[1]
                .parse::<u64>()
                .context("Invalid UTxO index")?;

            input_block.fields.push(tx3_lang::ast::InputBlockField::Ref(
                tx3_lang::ast::DataExpr::UtxoRef(tx3_lang::ast::UtxoRef {
                    txid,
                    index,
                    span: tx3_lang::ast::Span::default(),
                }),
            ));

            let min_amount = Text::new("Minimum amount value:")
                .with_default("1000000")
                .prompt()?;

            let min_amount_value = min_amount
                .parse::<i64>()
                .context("Invalid minimum amount value")?;

            input_block.fields.push(tx3_lang::ast::InputBlockField::MinAmount(
                tx3_lang::ast::DataExpr::StaticAssetConstructor(tx3_lang::ast::StaticAssetConstructor {
                    amount: Box::new(tx3_lang::ast::DataExpr::Number(min_amount_value)),
                    span: tx3_lang::ast::Span::default(),
                    r#type: tx3_lang::ast::Identifier::new("Ada".to_string()),
                })
            ));

            self.ast.txs[self.def_index].inputs.push(input_block);

            let add_more = Confirm::new("Add another input?")
                .with_default(false)
                .prompt()?;

            if !add_more {
                break;
            }
        }

        Ok(())
    }

    pub fn collect_outputs(&mut self, skip_question: bool) -> Result<()> {
        println!("\n📤 Transaction Outputs");
        println!("=====================");

        if !skip_question {
            let add_outputs = Confirm::new("Do you want to add outputs to your transaction?")
                .with_default(true)
                .prompt()?;
    
            if !add_outputs {
                return Ok(());
            }
        }

        loop {
            let has_name = Confirm::new("Does this output have a name?")
                .with_default(true)
                .prompt()?;

            let output_name = if has_name {
                Some(Text::new("Output name:")
                    .with_help_message("Enter output name")
                    .prompt()?)
            } else {
                None
            };

            let mut output_block = tx3_lang::ast::OutputBlock {
                name: output_name.as_ref().map(|name| tx3_lang::ast::Identifier::new(name.clone())),
                span: tx3_lang::ast::Span::default(),
                fields: Vec::new(),
            };

            let to_address = Text::new("To address:")
                .with_help_message("Enter the address this output goes to")
                .prompt()?;

            // Validate address
            let address = Address::from_str(&to_address)
                .context("Invalid address")?;

            let bech32 = address
                .to_bech32()
                .context("Failed to encode bech32 address")?;

            output_block.fields.push(tx3_lang::ast::OutputBlockField::To(
                Box::new(tx3_lang::ast::DataExpr::String(
                    tx3_lang::ast::StringLiteral::new(bech32)
                )),
            ));

            let amount = Text::new("Amount:")
                .with_default("1000000")
                .prompt()?;

            let amount_value = amount
                .parse::<i64>()
                .context("Invalid Ada amount")?;

            output_block.fields.push(tx3_lang::ast::OutputBlockField::Amount(
                Box::new(tx3_lang::ast::DataExpr::StaticAssetConstructor(tx3_lang::ast::StaticAssetConstructor {
                    amount: Box::new(tx3_lang::ast::DataExpr::Number(amount_value)),
                    span: tx3_lang::ast::Span::default(),
                    r#type: tx3_lang::ast::Identifier::new("Ada".to_string()),
                }))
            ));

            self.ast.txs[self.def_index].outputs.push(output_block);


            let add_more = Confirm::new("Add another output?")
                .with_default(true)
                .prompt()?;

            if !add_more {
                break;
            }
        }

        Ok(())
    }

    pub fn generate_tx3_content(self) -> String {
        let mut content = String::new();

        // Add transaction
        content.push_str(&format!("tx {}() {{\n", self.ast.txs[self.def_index].name.value));

        // Add inputs
        for input in &self.ast.txs[self.def_index].inputs {
            content.push_str(&format!("\tinput {} {{\n", input.name));
            input.fields.iter().for_each(|field| {
                match field {
                    tx3_lang::ast::InputBlockField::From(
                        tx3_lang::ast::DataExpr::String(literal)
                    ) => {
                        content.push_str(&format!("\t\tfrom: \"{}\",\n", literal.value));
                    },
                    tx3_lang::ast::InputBlockField::Ref(
                        tx3_lang::ast::DataExpr::UtxoRef(utxoref)
                    ) => {
                        content.push_str(&format!("\t\tref: 0x{}#{},\n", hex::encode(&utxoref.txid), utxoref.index));
                    },
                    tx3_lang::ast::InputBlockField::MinAmount(
                        tx3_lang::ast::DataExpr::StaticAssetConstructor(constructor)
                    ) => {
                        let amount = match *constructor.amount {
                            tx3_lang::ast::DataExpr::Number(num) => num.to_string(),
                            _ => "unknown".to_string(),
                        };
                        content.push_str(&format!("\t\tmin_amount: {}({}),\n", constructor.r#type.value, amount));
                    },
                    _ => {}
                }
            });
            content.push_str("\t}\n\n");
        }

        // Add outputs
        for output in &self.ast.txs[self.def_index].outputs {
            if let Some(name) = &output.name {
                content.push_str(&format!("\toutput {} {{\n", name.value));
            } else {
                content.push_str("\toutput {\n");
            }

            output.fields.iter().for_each(|field| {
                match field {
                    tx3_lang::ast::OutputBlockField::To(expr) => {
                        if let tx3_lang::ast::DataExpr::String(literal) = expr.as_ref() {
                            content.push_str(&format!("\t\tto: \"{}\",\n", literal.value));
                        }
                    },
                    tx3_lang::ast::OutputBlockField::Amount(expr) => {
                        if let tx3_lang::ast::DataExpr::StaticAssetConstructor(constructor) = expr.as_ref() {
                            let amount = match *constructor.amount {
                                tx3_lang::ast::DataExpr::Number(num) => num.to_string(),
                                _ => "unknown".to_string(),
                            };
                            content.push_str(&format!("\t\tamount: {}({}),\n", constructor.r#type.value, amount));
                        }
                    },
                    _ => {}
                }
            });
            content.push_str("\t}\n\n");
        }

        content.push_str("}\n");
        content
    }
}
