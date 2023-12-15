use anyhow::{bail, Result};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Password};
use io::edit_text;
use orion::kex;
use types::{
    app::App,
    secret::{ClearTextSecret, Secret},
    user,
};

mod cli;
mod crypto;
mod db;
mod io;
mod types;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Init => {
            let db_exists = db::exists().await?;

            if db_exists {
                bail!("A database already exists at {}", db::db_path()?);
            }

            let master_password: String = Password::with_theme(&ColorfulTheme::default())
                .with_prompt("Set a master password")
                .with_confirmation("Confirm password", "Passwords do not match")
                .interact()?;

            let user = user::User::new(&master_password)?;

            let db = db::init().await?;

            user.store(&db).await?;
        }
        cli::Command::Create { name, description } => {
            let app = App::new().await?;

            let value = edit_text(b"")?;
            let value_bytes = std::str::from_utf8(&value)?;

            let sec = ClearTextSecret::new(&name, value_bytes, description);
            let encrypted = sec.to_encrypted(&app.key)?;
            if let Err(e) = encrypted.store(&app.db).await {
                eprintln!("{}", e);
            }
        }
        cli::Command::Get { name, json } => {
            let app = App::new().await?;

            let sec = Secret::get(&app.db, &name).await?;
            let cleartext = sec.to_cleartext(&app.key)?;

            if json {
                println!("{}", cleartext.to_json()?)
            } else {
                println!("{}", cleartext.value)
            }
        }
        cli::Command::Edit { name } => {
            let app = App::new().await?;

            let mut sec = Secret::get(&app.db, &name).await?;
            let clear_text = crypto::decrypt_bytes(&app.key, &sec.value)?;

            let new_contents = edit_text(&clear_text)?;

            if new_contents == clear_text {
                println!("Secret not changed. Aborting...")
            } else {
                let new_encrypted = crypto::encrypt_bytes(&app.key, &new_contents)?;
                sec.value = new_encrypted;
                sec.update(&app.db).await?;

                println!("Updated secret {}", sec.name);
            }
        }
        cli::Command::Delete { name } => {
            let app = App::new().await?;

            let sec = Secret::get(&app.db, &name).await?;
            sec.delete(&app.db).await?;
        }
        cli::Command::List => {
            let app = App::new().await?;

            let secrets = Secret::get_all(&app.db).await?;
            for secret in secrets {
                println!(
                    "{}\t\t{}",
                    secret.name,
                    secret.description.unwrap_or_default()
                )
            }
        }
        cli::Command::Session => {
            let db = db::connect().await?;
            let input_password = App::read_password()?;
            let user = App::authenticate_user(&db, &input_password).await?;
            let key = kex::SecretKey::generate(256)?;
            let encrypted_password = crypto::encrypt_bytes(&key, input_password.as_bytes())?;
            let id = uuid::Uuid::new_v4();
        }
    }

    Ok(())
}
