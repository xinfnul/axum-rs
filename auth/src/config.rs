use std::{env, net::SocketAddr, str::FromStr};

use anyhow::{Result, anyhow, bail};
use axum_extra::extract::cookie::SameSite;
use chrono::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: SocketAddr,
    pub cors_origin: String,
    pub app_env: AppEnv,

    pub database_url: String,

    pub jwt_refresh_secret: String,
    pub jwt_access_secret: String,

    pub refresh_token_ttl: Duration,
    pub access_token_ttl: Duration,

    pub cookie_same_site: axum_extra::extract::cookie::SameSite,
    pub cookie_secure: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Development,
    Production,
    Test,
}

impl FromStr for AppEnv {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            "test" => Ok(Self::Test),
            _ => bail!("APP_ENV must be development, test, or production"),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let server_addr: SocketAddr =
            parse_or("SERVER_ADDR", SocketAddr::from(([127, 0, 0, 1], 5000)))?;

        let cors_origin = parse_or("CORS_ORIGIN", "http://localhost:3000".to_owned())?;

        let app_env: AppEnv = parse_or("APP_ENV", AppEnv::Development)?;

        let database_url = required("DATABASE_URL")?;
        let jwt_access_secret = required("JWT_ACCESS_SECRET")?;
        let jwt_refresh_secret = required("JWT_REFRESH_SECRET")?;

        validate_secret("JWT_ACCESS_SECRET", &jwt_access_secret)?;
        validate_secret("JWT_REFRESH_SECRET", &jwt_refresh_secret)?;

        if jwt_access_secret == jwt_refresh_secret {
            bail!("JWT access and refresh tokens must be different");
        }

        let access_token_ttl = Duration::seconds(parse_or("ACCESS_TOKEN_TTL", 600i64)?);
        let refresh_token_ttl = Duration::seconds(parse_or("ACCESS_TOKEN_TTL", 1_209_600_i64)?);

        if access_token_ttl.is_zero() || refresh_token_ttl.is_zero() {
            bail!("Token TTLs must be greater than zero");
        }

        let cookie_same_site: SameSite = match env::var("COOKIE_SAME_SITE") {
            Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
                "strict" => SameSite::Strict,
                "lax" => SameSite::Lax,
                "none" => SameSite::None,
                _ => bail!("COOKIE_SAME_SITE must be strict, lax, or none"),
            },
            Err(env::VarError::NotPresent) => SameSite::Lax,
            Err(error) => return Err(error.into()),
        };

        let cookie_secure: bool = match env::var("COOKIE_SECURE") {
            Ok(value) => parse_bool("COOKIE_SECURE", &value)?,
            Err(env::VarError::NotPresent) => app_env == AppEnv::Production,
            Err(error) => return Err(error.into()),
        };

        if cookie_same_site == SameSite::None && !cookie_secure {
            bail!("SameSite=None requires COOKIE_SECURE=true");
        }

        Ok(Self {
            server_addr,
            cors_origin,
            app_env,
            database_url,
            jwt_access_secret,
            jwt_refresh_secret,
            access_token_ttl,
            refresh_token_ttl,
            cookie_same_site,
            cookie_secure,
        })
    }
}

fn required(key: &str) -> Result<String> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) => bail!("{key} must not be empty"),
        Err(env::VarError::NotPresent) => {
            bail!("Missing required environment variable: {key}");
        }
        Err(error) => Err(error.into()),
    }
}

fn parse_or<T>(key: &str, default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let value = match env::var(key) {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => return Ok(default),
        Err(error) => return Err(error.into()),
    };

    value
        .parse()
        .map_err(|error| anyhow!("Invalid {key}: {error}"))
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => bail!("{key} must be true, false, 1, or 0"),
    }
}

fn validate_secret(key: &str, secret: &str) -> Result<()> {
    if secret.len() < 32 {
        bail!("{key} must contain at least 32 bytes.");
    }

    Ok(())
}
