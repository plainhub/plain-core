// PKCS#12 name abbreviations from x509-parser and oid-registry (SOURCE.json).
use der::asn1::ObjectIdentifier;

pub(super) fn short_name(oid: &ObjectIdentifier) -> Option<&'static str> {
    if let [0x55, 0x04, id] = oid.as_bytes() {
        return X520.get(usize::from(*id)).copied().flatten();
    }
    NAMES
        .binary_search_by(|(key, _)| key.as_bytes().cmp(oid.as_bytes()))
        .ok()
        .map(|index| NAMES[index].1)
}

const fn oid(value: &str) -> ObjectIdentifier {
    ObjectIdentifier::new_unwrap(value)
}

const X520: [Option<&str>; 47] = {
    let mut names = [None; 47];
    names[0] = Some("objectClass");
    names[1] = Some("aliasedEntryName");
    names[2] = Some("knowledgeInformation");
    names[3] = Some("CN");
    names[4] = Some("surname");
    names[5] = Some("serialNumber");
    names[6] = Some("C");
    names[7] = Some("L");
    names[8] = Some("ST");
    names[9] = Some("streetAddress");
    names[10] = Some("O");
    names[11] = Some("OU");
    names[12] = Some("title");
    names[13] = Some("description");
    names[14] = Some("searchGuide");
    names[15] = Some("businessCategory");
    names[16] = Some("postalAddress");
    names[17] = Some("postalCode");
    names[41] = Some("name");
    names[42] = Some("givenName");
    names[43] = Some("initials");
    names[44] = Some("generationQualifier");
    names[45] = Some("uniqueIdentifier");
    names[46] = Some("dnQualifier");
    names
};

const NAMES: &[(ObjectIdentifier, &str)] = &[
    (oid("0.9.2342.19200300.100.1.1"), "uid"),
    (oid("0.9.2342.19200300.100.1.25"), "DC"),
    (oid("1.2.643.2.2.3"), "id-GostR3411-94-with-GostR3410-2001"),
    (oid("1.2.643.2.2.19"), "gostR3410-2001"),
    (oid("1.2.643.7.1.1.1.1"), "gost3410-2012-256"),
    (oid("1.2.643.7.1.1.1.2"), "gost3410-2012-512"),
    (
        oid("1.2.643.7.1.1.3.2"),
        "id-tc26-signwithdigest-gost3410-12-256",
    ),
    (
        oid("1.2.643.7.1.1.3.3"),
        "id-tc26-signwithdigest-gost3410-12-512",
    ),
    (oid("1.2.840.113549.1.1.1"), "rsaEncryption"),
    (oid("1.2.840.113549.1.1.2"), "md2WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.3"), "md4WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.4"), "md5WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.5"), "sha1WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.8"), "id-mgf1"),
    (oid("1.2.840.113549.1.1.10"), "rsassa-pss"),
    (oid("1.2.840.113549.1.1.11"), "sha256WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.12"), "sha384WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.13"), "sha512WithRSAEncryption"),
    (oid("1.2.840.113549.1.1.14"), "sha224WithRSAEncryption"),
    (oid("1.2.840.113549.1.7.1"), "pkcs7-data"),
    (oid("1.2.840.113549.1.7.2"), "pkcs7-signedData"),
    (oid("1.2.840.113549.1.7.3"), "pkcs7-envelopedData"),
    (oid("1.2.840.113549.1.7.4"), "pkcs7-signedAndEnvelopedData"),
    (oid("1.2.840.113549.1.7.5"), "pkcs7-digestedData"),
    (oid("1.2.840.113549.1.7.6"), "pkcs7-encryptedData"),
    (oid("1.2.840.113549.1.9.1"), "Email"),
    (oid("1.2.840.113549.1.9.2"), "unstructuredName"),
    (oid("1.2.840.113549.1.9.3"), "contentType"),
    (oid("1.2.840.113549.1.9.4"), "id-messageDigest"),
    (oid("1.2.840.113549.1.9.5"), "signing-time"),
    (oid("1.2.840.113549.1.9.7"), "challengePassword"),
    (oid("1.2.840.113549.1.9.14"), "extensionRequest"),
    (oid("1.2.840.113549.1.9.15"), "smimeCapabilities"),
    (oid("1.2.840.113549.1.9.20"), "friendlyName"),
    (oid("1.2.840.113549.1.12"), "pkcs-12"),
    (oid("1.2.840.113549.1.12.1"), "pkcs-12PbeIds"),
    (oid("1.2.840.113549.1.12.1.1"), "pbeWithSHAAnd128BitRC4"),
    (oid("1.2.840.113549.1.12.1.2"), "pbeWithSHAAnd40BitRC4"),
    (
        oid("1.2.840.113549.1.12.1.3"),
        "pbeWithSHAAnd3-KeyTripleDES-CBC",
    ),
    (
        oid("1.2.840.113549.1.12.1.4"),
        "pbeWithSHAAnd2-KeyTripleDES-CBC",
    ),
    (oid("1.2.840.113549.1.12.1.5"), "pbeWithSHAAnd128BitRC2-CBC"),
    (oid("1.2.840.113549.1.12.1.6"), "pbeWithSHAAnd40BitRC2-CBC"),
    (oid("1.2.840.10040.4.1"), "id-dsa"),
    (oid("1.2.840.10040.4.3"), "dsa-with-sha1"),
    (oid("1.2.840.10045.2.1"), "id-ecPublicKey"),
    (oid("1.2.840.10045.3.1.7"), "prime256v1"),
    (oid("1.2.840.10045.4.3.1"), "ecdsa-with-SHA224"),
    (oid("1.2.840.10045.4.3.2"), "ecdsa-with-SHA256"),
    (oid("1.2.840.10045.4.3.3"), "ecdsa-with-SHA384"),
    (oid("1.2.840.10045.4.3.4"), "ecdsa-with-SHA512"),
    (oid("1.3.6.1.4.1.311.60.2.1.1"), "msJurisdictionLocality"),
    (
        oid("1.3.6.1.4.1.311.60.2.1.2"),
        "msJurisdictionStateOrProvince",
    ),
    (oid("1.3.6.1.4.1.311.60.2.1.3"), "msJurisdictionCountry"),
    (oid("1.3.6.1.4.1.11129.2.4.2"), "ctSCTList"),
    (oid("1.3.6.1.5.5.7.1.1"), "authorityInfoAccess"),
    (oid("1.3.6.1.5.5.7.1.11"), "subjectInfoAccess"),
    (oid("1.3.6.1.5.5.7.48.1"), "id-ad-ocsp"),
    (oid("1.3.6.1.5.5.7.48.2"), "id-ad-caIssuers"),
    (oid("1.3.6.1.5.5.7.48.3"), "id-ad-timestamping"),
    (oid("1.3.6.1.5.5.7.48.4"), "id-ad-dvcs"),
    (oid("1.3.6.1.5.5.7.48.5"), "id-ad-caRepository"),
    (oid("1.3.6.1.5.5.7.48.6"), "id-ad-http-certs"),
    (oid("1.3.6.1.5.5.7.48.7"), "id-ad-http-crls"),
    (oid("1.3.6.1.5.5.7.48.10"), "id-ad-rpki-manifest"),
    (oid("1.3.6.1.5.5.7.48.11"), "id-ad-signed-object"),
    (oid("1.3.6.1.5.5.7.48.12"), "id-ad-cmc"),
    (oid("1.3.6.1.5.5.7.48.13"), "id-ad-rpki-notify"),
    (oid("1.3.6.1.5.5.7.48.14"), "id-ad-stirTNList"),
    (oid("1.3.14.3.2.25"), "md5WithRSASignature"),
    (oid("1.3.14.3.2.26"), "id-SHA1"),
    (oid("1.3.14.3.2.29"), "sha1WithRSAEncryption"),
    (oid("1.3.36.3.3.1.2"), "rsaSignatureWithripemd160"),
    (oid("1.3.101.112"), "ed25519"),
    (oid("1.3.101.113"), "ed448"),
    (oid("1.3.132.0.34"), "secp384r1"),
    (oid("1.3.132.0.35"), "secp521r1"),
    (
        oid("1.3.133.16.840.63.0.2"),
        "dhSinglePass-stdDH-sha1kdf-scheme",
    ),
    (oid("2.5.4"), "x509"),
    (oid("2.5.29.1"), "oldAuthorityKeyIdentifier"),
    (oid("2.5.29.2"), "oldKeyAttributes"),
    (oid("2.5.29.3"), "oldCertificatePolicies"),
    (oid("2.5.29.4"), "oldKeyUsage"),
    (oid("2.5.29.5"), "oldPolicyMapping"),
    (oid("2.5.29.6"), "oldSubtreesConstraint"),
    (oid("2.5.29.7"), "oldSubjectAltNAme"),
    (oid("2.5.29.8"), "oldIssuerAltNAme"),
    (oid("2.5.29.14"), "subjectKeyIdentifier"),
    (oid("2.5.29.15"), "keyUsage"),
    (oid("2.5.29.16"), "privateKeyUsagePeriod"),
    (oid("2.5.29.17"), "subjectAltName"),
    (oid("2.5.29.18"), "issuerAltName"),
    (oid("2.5.29.19"), "basicConstraints"),
    (oid("2.5.29.20"), "crlNumber"),
    (oid("2.5.29.21"), "reasonCode"),
    (oid("2.5.29.23"), "holdInstructionCode"),
    (oid("2.5.29.24"), "invalidityDate"),
    (oid("2.5.29.27"), "deltaCRLIndicator"),
    (oid("2.5.29.28"), "issuerDistributionPoint"),
    (oid("2.5.29.29"), "issuer"),
    (oid("2.5.29.30"), "nameConstraints"),
    (oid("2.5.29.31"), "crlDistributionPoints"),
    (oid("2.5.29.32"), "certificatePolicies"),
    (oid("2.5.29.33"), "policyMappings"),
    (oid("2.5.29.35"), "authorityKeyIdentifier"),
    (oid("2.5.29.36"), "policyConstraints"),
    (oid("2.5.29.37"), "extendedKeyUsage"),
    (oid("2.5.29.46"), "freshestCRL"),
    (oid("2.5.29.54"), "inhibitAnyPolicy"),
    (oid("2.16.840.1.101.3.4.1.42"), "aes-256-cbc"),
    (oid("2.16.840.1.101.3.4.2.1"), "sha256"),
    (oid("2.16.840.1.101.3.4.2.2"), "sha384"),
    (oid("2.16.840.1.101.3.4.2.3"), "sha512"),
    (oid("2.16.840.1.113730.1.1"), "nsCertType"),
    (oid("2.16.840.1.113730.1.2"), "nsBaseURL"),
    (oid("2.16.840.1.113730.1.3"), "nsRevocationURL"),
    (oid("2.16.840.1.113730.1.4"), "nsCARevocationURL"),
    (oid("2.16.840.1.113730.1.5"), "nsCACRLURL"),
    (oid("2.16.840.1.113730.1.6"), "nsCACertURL"),
    (oid("2.16.840.1.113730.1.7"), "nsRenewalURL"),
    (oid("2.16.840.1.113730.1.8"), "nsCAPolicyURL"),
    (oid("2.16.840.1.113730.1.9"), "nsHomepageURL"),
    (oid("2.16.840.1.113730.1.10"), "nsEntityLogo"),
    (oid("2.16.840.1.113730.1.11"), "nsUserPicture"),
    (oid("2.16.840.1.113730.1.12"), "nsSSLServerName"),
    (oid("2.16.840.1.113730.1.13"), "nsComment"),
];
