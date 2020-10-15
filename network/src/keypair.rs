use libp2p::identity::Keypair;
use std::{
    error::Error,
    process::{Command, Stdio},
};

pub fn keypair_from_openssl_rsa() -> Result<Keypair, Box<dyn Error>> {
    // This is a temporary hack.
    // I'm using an RSA keypair because the new elliptic curve
    // based identity seems to break p2p-circuit routing in go-ipfs?

    log::info!("generating key pair");

    let mut genpkey = Command::new("openssl")
        .arg("genpkey")
        .arg("-algorithm")
        .arg("RSA")
        .arg("-pkeyopt")
        .arg("rsa_keygen_bits:2048")
        .arg("-pkeyopt")
        .arg("rsa_keygen_pubexp:65537")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let pkcs8 = Command::new("openssl")
        .arg("pkcs8")
        .arg("-topk8")
        .arg("-nocrypt")
        .arg("-outform")
        .arg("der")
        .stdin(Stdio::from(genpkey.stdout.take().unwrap()))
        .output()
        .unwrap();

    let mut data = pkcs8.stdout.as_slice().to_vec();
    Ok(Keypair::rsa_from_pkcs8(&mut data)?)
}
