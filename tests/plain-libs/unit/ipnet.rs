use super::*;
#[test]
fn cidr_boundaries_and_old_accepted_notation_are_preserved() {
    for (cidr, inside, outside) in [
        ("192.000.002.127/24", "192.0.2.255", "192.0.3.0"),
        (
            "::ffff:192.0.2.1/128",
            "::ffff:192.0.2.1",
            "::ffff:192.0.2.2",
        ),
        ("::/0", "ffff:ffff::1", "192.0.2.1"),
        ("0.0.0.0/0", "255.255.255.255", "::1"),
    ] {
        let network = cidr.parse::<IpNet>().unwrap();
        assert!(network.contains(&inside.parse().unwrap()));
        assert!(!network.contains(&outside.parse().unwrap()));
    }
    for bad in [
        "1.2.3.4/33",
        "::/129",
        "1.2.3.4/-1",
        "1.2.3.4/24junk",
        "1.2.3/24",
    ] {
        assert!(bad.parse::<IpNet>().is_err());
    }
}
