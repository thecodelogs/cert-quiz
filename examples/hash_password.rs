use argon2::password_hash::PasswordHasher;
use argon2::Argon2;

fn main() {
    let password = std::env::args()
        .nth(1)
        .expect("usage: cargo run --example hash_password -- <password>");
    let hash = Argon2::default()
        .hash_password(password.as_bytes())
        .expect("hashing failed")
        .to_string();
    println!("{hash}");
}
