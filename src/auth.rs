use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, errors::Error as JwtError};
use serde::{Serialize, Deserialize};

const JWT_SECRET: &[u8] = b"Jwt_Secret";

/// 创建token
pub fn create_jwt(id: &i32) -> String {
  let expiration = Utc::now()
      .checked_add_signed(chrono::Duration::seconds(3600))
      .expect("valid timestamp")
      .timestamp();

  let header = Header::new(Algorithm::HS512);
  let claims = Claims::new(id, expiration as usize);

  jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(JWT_SECRET))
      .map(|s| format!("Bearer {}", s))
      .unwrap()
}

/// 验证token
pub fn validate_token(token: &str) -> Result<TokenData<Claims>, JwtError> {
  let validation = Validation::new(Algorithm::HS512);
  let key = DecodingKey::from_secret(JWT_SECRET);
  let data = jsonwebtoken::decode::<Claims>(token, &key, &validation)?;
  Ok(data)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Claims {
    iss: String,
    pub exp: usize,
    /// 保存的用户id
    pub id: i32,
}

impl Claims {
    pub fn new(id: &i32, exp: usize) -> Self {
        Self {
            iss: "test".to_owned(),
            id: *id,
            exp,
        }
    }
}