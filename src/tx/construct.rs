use std::{fs, path::PathBuf, str::FromStr};

use anyhow::{Result, Context};
use clap::Parser;
use pallas::ledger::addresses::Address;
use serde_json::json;
use tracing::instrument;
use tx3_lang::Protocol;
use inquire::{Text, Confirm};

#[derive(Parser, Clone)]
pub struct Args {
    /// Path for tx3 file to create the transaction
    #[arg(long)]
    tx3_file: PathBuf,
}

struct TransactionBuilder {
    ast: tx3_lang::ast::Program,
    def_index: usize,
}

#[instrument("construct", skip_all)]
pub async fn run(args: Args, _ctx: &crate::Context) -> Result<()> {
    tracing::debug!("Creating transaction from {}", args.tx3_file.display());
    let ast_path_buf = args.tx3_file.with_extension("ast");

    // let ast = if args.tx3_file.exists() {
    //     let protocol = Protocol::from_file(&args.tx3_file)
    //         .load()
    //         .context("Failed to load existing tx3 file")?;
    //     protocol.ast().clone()
    // } else if ast_path_buf.exists() {
    //     let ast_content = fs::read_to_string(&ast_path_buf)
    //         .context("Failed to read existing AST file")?;

    //     dbg!("Loaded existing AST from {}", ast_path_buf.display());

    //     serde_json::from_str(&ast_content)
    //         .context("Failed to parse existing AST file")?
    // } else {
    //     tx3_lang::ast::Program::default()
    // };

    let ast = if ast_path_buf.exists() {
        let ast_content = fs::read_to_string(&ast_path_buf)
            .context("Failed to read existing AST file")?;

        dbg!("Loaded existing AST from {}", ast_path_buf.display());

        serde_json::from_str(&ast_content)
            .context("Failed to parse existing AST file")?
    } else {
        tx3_lang::ast::Program::default()
    };

    dbg!("Initial AST: {:#?}", &ast);

    let mut tx_builder = TransactionBuilder::new("new_transaction".to_string(), ast);

    tx_builder.collect_inputs()?;

    tx_builder.collect_outputs()?;

    let ast = tx_builder.ast.clone();

    // Generate the tx3 content
    let tx3_content = tx_builder.generate_tx3_content();

    // Write to file
    fs::write(&args.tx3_file, tx3_content)
        .context("Failed to write tx3 file")?;

    fs::write(ast_path_buf, serde_json::to_string_pretty(&ast).unwrap())
        .context("Failed to write tx3 AST file")?;

    println!("\n✅ Transaction created successfully!");
    println!("📄 File saved to: {}", args.tx3_file.display());

    Ok(())
}

impl TransactionBuilder {
    fn new(name: String, mut ast: tx3_lang::ast::Program) -> Self {
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
            ast.txs.push(serde_json::from_value(value).unwrap());

            def_index = Some(ast.txs.len() - 1);
        }
        
        Self {
            ast: ast.clone(),
            def_index: def_index.unwrap(),
        }
    }

    fn collect_inputs(&mut self) -> Result<()> {
        println!("\n📥 Transaction Inputs");
        println!("====================");

        let add_inputs = Confirm::new("Do you want to add inputs to your transaction?")
            .with_default(true)
            .prompt()?;

        if !add_inputs {
            return Ok(());
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

            let from_address = Text::new("From address:")
                .with_help_message("Enter the address this input comes from")
                .prompt()?;

            // Validate address
            let address = Address::from_str(&from_address)
                .context("Invalid address")?;

            input_block.fields.push(tx3_lang::ast::InputBlockField::From(
                tx3_lang::ast::DataExpr::String(tx3_lang::ast::StringLiteral::new(address.to_bech32().unwrap())),
            ));

            let min_amount = Text::new("Minimum amount value:")
                .with_default("1000000")
                .prompt()?;

            input_block.fields.push(tx3_lang::ast::InputBlockField::MinAmount(
                tx3_lang::ast::DataExpr::StaticAssetConstructor(tx3_lang::ast::StaticAssetConstructor {
                    amount: Box::new(tx3_lang::ast::DataExpr::Number(min_amount.parse::<i64>().unwrap())),
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

    fn collect_outputs(&mut self) -> Result<()> {
        println!("\n📤 Transaction Outputs");
        println!("=====================");

        let add_outputs = Confirm::new("Do you want to add outputs to your transaction?")
            .with_default(true)
            .prompt()?;

        if !add_outputs {
            return Ok(());
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
                name: if let Some(name) = &output_name {
                    Some(tx3_lang::ast::Identifier::new(name.clone()))
                } else {
                    None
                },
                span: tx3_lang::ast::Span::default(),
                fields: Vec::new(),
            };

            let to_address = Text::new("To address:")
                .with_help_message("Enter the address this output goes to")
                .prompt()?;

            // Validate address
            let address = Address::from_str(&to_address)
                .context("Invalid address")?;

            output_block.fields.push(tx3_lang::ast::OutputBlockField::To(
                Box::new(tx3_lang::ast::DataExpr::String(tx3_lang::ast::StringLiteral::new(address.to_bech32().unwrap()))),
            ));

            let amount = Text::new("Amount:")
                .with_default("1000000")
                .prompt()?;

            output_block.fields.push(tx3_lang::ast::OutputBlockField::Amount(
                Box::new(tx3_lang::ast::DataExpr::StaticAssetConstructor(tx3_lang::ast::StaticAssetConstructor {
                    amount: Box::new(tx3_lang::ast::DataExpr::Number(amount.parse::<i64>().unwrap())),
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

    fn generate_tx3_content(self) -> String {
        let mut content = String::new();

        // Add transaction
        content.push_str(&format!("tx {}() {{\n", self.ast.txs[self.def_index].name.value));

        // Add inputs
        for input in &self.ast.txs[self.def_index].inputs {
            content.push_str(&format!("\tinput {} {{\n", input.name));
            input.fields.iter().for_each(|field| {
                match field {
                    tx3_lang::ast::InputBlockField::From(expr) => {
                        match expr {
                            tx3_lang::ast::DataExpr::String(literal) => {
                                content.push_str(&format!("\t\tfrom: \"{}\",\n", literal.value));
                            }
                            _ => {}
                        }
                    },
                    tx3_lang::ast::InputBlockField::MinAmount(expr) => {
                        match expr {
                            tx3_lang::ast::DataExpr::StaticAssetConstructor(constructor) => {
                                let amount = match *constructor.amount {
                                    tx3_lang::ast::DataExpr::Number(num) => num.to_string(),
                                    _ => "unknown".to_string(),
                                };
                                content.push_str(&format!("\t\tmin_amount: {}({}),\n", constructor.r#type.value, amount));
                            }
                            _ => {}
                        }
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
                        match expr.as_ref() {
                            tx3_lang::ast::DataExpr::String(literal) => {
                                content.push_str(&format!("\t\tto: \"{}\",\n", literal.value));
                            }
                            _ => {}
                        }
                    },
                    tx3_lang::ast::OutputBlockField::Amount(expr) => {
                        match expr.as_ref() {
                            tx3_lang::ast::DataExpr::StaticAssetConstructor(constructor) => {
                                let amount = match *constructor.amount {
                                    tx3_lang::ast::DataExpr::Number(num) => num.to_string(),
                                    _ => "unknown".to_string(),
                                };
                                content.push_str(&format!("\t\tamount: {}({}),\n", constructor.r#type.value, amount));
                            }
                            _ => {}
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
