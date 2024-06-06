use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, format_err, Error};
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::x509::{X509Builder, X509};

use proxmox_acme::async_client::AcmeClient;
use proxmox_rest_server::WorkerTask;
use proxmox_sys::{task_log, task_warn};

use crate::types::{AcmeConfig, AcmeDomain};

pub async fn revoke_certificate(acme_config: &AcmeConfig, certificate: &[u8]) -> Result<(), Error> {
    let mut acme = super::account_config::load_account_config(&acme_config.account)
        .await?
        .client();

    acme.revoke_certificate(certificate, None).await?;

    Ok(())
}

pub struct OrderedCertificate {
    pub certificate: Vec<u8>,
    pub private_key_pem: Vec<u8>,
}

pub async fn order_certificate(
    worker: Arc<WorkerTask>,
    acme_config: &AcmeConfig,
    domains: &[AcmeDomain],
) -> Result<Option<OrderedCertificate>, Error> {
    use proxmox_acme::authorization::Status;
    use proxmox_acme::order::Identifier;

    let get_domain_config = |domain: &str| {
        domains
            .iter()
            .find(|d| d.domain == domain)
            .ok_or_else(|| format_err!("no config for domain '{}'", domain))
    };

    if domains.is_empty() {
        task_log!(
            worker,
            "No domains configured to be ordered from an ACME server."
        );
        return Ok(None);
    }

    let mut acme = super::account_config::load_account_config(&acme_config.account)
        .await?
        .client();

    let (plugins, _) = super::plugin_config::plugin_config()?;

    task_log!(worker, "Placing ACME order");

    let order = acme
        .new_order(domains.iter().map(|d| d.domain.to_ascii_lowercase()))
        .await?;

    task_log!(worker, "Order URL: {}", order.location);

    let identifiers: Vec<String> = order
        .data
        .identifiers
        .iter()
        .map(|identifier| match identifier {
            Identifier::Dns(domain) => domain.clone(),
        })
        .collect();

    for auth_url in &order.data.authorizations {
        task_log!(worker, "Getting authorization details from '{}'", auth_url);
        let mut auth = acme.get_authorization(auth_url).await?;

        let domain = match &mut auth.identifier {
            Identifier::Dns(domain) => domain.to_ascii_lowercase(),
        };

        if auth.status == Status::Valid {
            task_log!(worker, "{} is already validated!", domain);
            continue;
        }

        task_log!(worker, "The validation for {} is pending", domain);
        let domain_config: &AcmeDomain = get_domain_config(&domain)?;
        let plugin_id = domain_config.plugin.as_deref().unwrap_or("standalone");
        let mut plugin_cfg =
            crate::acme_plugin::get_acme_plugin(&plugins, plugin_id)?.ok_or_else(|| {
                format_err!("plugin '{}' for domain '{}' not found!", plugin_id, domain)
            })?;

        task_log!(worker, "Setting up validation plugin");
        let validation_url = plugin_cfg
            .setup(&mut acme, &auth, domain_config, Arc::clone(&worker))
            .await?;

        let result = request_validation(&worker, &mut acme, auth_url, validation_url).await;

        if let Err(err) = plugin_cfg
            .teardown(&mut acme, &auth, domain_config, Arc::clone(&worker))
            .await
        {
            task_warn!(
                worker,
                "Failed to teardown plugin '{}' for domain '{}' - {}",
                plugin_id,
                domain,
                err
            );
        }

        result?;
    }

    task_log!(worker, "All domains validated");
    task_log!(worker, "Creating CSR");

    let csr = proxmox_acme::util::Csr::generate(&identifiers, &Default::default())?;
    let mut finalize_error_cnt = 0u8;
    let order_url = &order.location;
    let mut order;
    loop {
        use proxmox_acme::order::Status;

        order = acme.get_order(order_url).await?;

        match order.status {
            Status::Pending => {
                task_log!(worker, "still pending, trying to finalize anyway");
                let finalize = order
                    .finalize
                    .as_deref()
                    .ok_or_else(|| format_err!("missing 'finalize' URL in order"))?;
                if let Err(err) = acme.finalize(finalize, &csr.data).await {
                    if finalize_error_cnt >= 5 {
                        return Err(err);
                    }

                    finalize_error_cnt += 1;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Status::Ready => {
                task_log!(worker, "order is ready, finalizing");
                let finalize = order
                    .finalize
                    .as_deref()
                    .ok_or_else(|| format_err!("missing 'finalize' URL in order"))?;
                acme.finalize(finalize, &csr.data).await?;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Status::Processing => {
                task_log!(worker, "still processing, trying again in 30 seconds");
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            Status::Valid => {
                task_log!(worker, "valid");
                break;
            }
            other => bail!("order status: {:?}", other),
        }
    }

    task_log!(worker, "Downloading certificate");
    let certificate = acme
        .get_certificate(
            order
                .certificate
                .as_deref()
                .ok_or_else(|| format_err!("missing certificate url in finalized order"))?,
        )
        .await?;

    Ok(Some(OrderedCertificate {
        certificate: certificate.to_vec(),
        private_key_pem: csr.private_key_pem,
    }))
}

async fn request_validation(
    worker: &WorkerTask,
    acme: &mut AcmeClient,
    auth_url: &str,
    validation_url: &str,
) -> Result<(), Error> {
    task_log!(worker, "Triggering validation");
    acme.request_challenge_validation(validation_url).await?;

    task_log!(worker, "Sleeping for 5 seconds");
    tokio::time::sleep(Duration::from_secs(5)).await;

    loop {
        use proxmox_acme::authorization::Status;

        let auth = acme.get_authorization(auth_url).await?;
        match auth.status {
            Status::Pending => {
                task_log!(
                    worker,
                    "Status is still 'pending', trying again in 10 seconds"
                );
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Status::Valid => return Ok(()),
            other => bail!(
                "validating challenge '{}' failed - status: {:?}",
                validation_url,
                other
            ),
        }
    }
}

pub fn create_self_signed_cert(
    product_name: &str,
    nodename: &str,
    domain: Option<&str>,
) -> Result<(PKey<Private>, X509), Error> {
    let rsa = Rsa::generate(4096).unwrap();

    let mut x509 = X509Builder::new()?;

    x509.set_version(2)?;

    let today = openssl::asn1::Asn1Time::days_from_now(0)?;
    x509.set_not_before(&today)?;
    let expire = openssl::asn1::Asn1Time::days_from_now(365 * 1000)?;
    x509.set_not_after(&expire)?;

    let mut fqdn = nodename.to_owned();

    if let Some(domain) = domain {
        fqdn.push('.');
        fqdn.push_str(domain);
    }

    // we try to generate an unique 'subject' to avoid browser problems
    //(reused serial numbers, ..)
    let uuid = proxmox_uuid::Uuid::generate();

    let mut subject_name = openssl::x509::X509NameBuilder::new()?;
    subject_name.append_entry_by_text("O", product_name)?;
    subject_name.append_entry_by_text("OU", &format!("{:X}", uuid))?;
    subject_name.append_entry_by_text("CN", &fqdn)?;
    let subject_name = subject_name.build();

    x509.set_subject_name(&subject_name)?;
    x509.set_issuer_name(&subject_name)?;

    let bc = openssl::x509::extension::BasicConstraints::new(); // CA = false
    let bc = bc.build()?;
    x509.append_extension(bc)?;

    let usage = openssl::x509::extension::ExtendedKeyUsage::new()
        .server_auth()
        .build()?;
    x509.append_extension(usage)?;

    let context = x509.x509v3_context(None, None);

    let mut alt_names = openssl::x509::extension::SubjectAlternativeName::new();

    alt_names.ip("127.0.0.1");
    alt_names.ip("::1");

    alt_names.dns("localhost");

    if nodename != "localhost" {
        alt_names.dns(nodename);
    }
    if nodename != fqdn {
        alt_names.dns(&fqdn);
    }

    let alt_names = alt_names.build(&context)?;

    x509.append_extension(alt_names)?;

    let pub_pem = rsa.public_key_to_pem()?;
    let pubkey = PKey::public_key_from_pem(&pub_pem)?;

    x509.set_pubkey(&pubkey)?;

    let context = x509.x509v3_context(None, None);
    let ext = openssl::x509::extension::SubjectKeyIdentifier::new().build(&context)?;
    x509.append_extension(ext)?;

    let context = x509.x509v3_context(None, None);
    let ext = openssl::x509::extension::AuthorityKeyIdentifier::new()
        .keyid(true)
        .build(&context)?;
    x509.append_extension(ext)?;

    let privkey = PKey::from_rsa(rsa)?;

    x509.sign(&privkey, openssl::hash::MessageDigest::sha256())?;

    Ok((privkey, x509.build()))
}
