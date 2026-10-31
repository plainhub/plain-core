use super::*;

#[test]
fn channel_actions_share_the_same_owner_and_membership_policy() {
    for status in [
        ChannelStatus::Joined,
        ChannelStatus::Left,
        ChannelStatus::Kicked,
    ] {
        let owner = actions(Some("owner"), status, "owner");
        assert!(owner.is_owner && owner.can_rename && owner.can_manage_members && owner.can_delete);
        assert!(!owner.can_leave);
        let member = actions(Some("owner"), status, "member");
        assert!(!member.is_owner && !member.can_rename && !member.can_manage_members);
        assert_eq!(member.can_leave, status == ChannelStatus::Joined);
        assert_eq!(member.can_delete, status != ChannelStatus::Joined);
        let missing = actions(None, status, "");
        assert!(
            !missing.is_owner
                && !missing.can_leave
                && !missing.can_delete
                && !missing.can_rename
                && !missing.can_manage_members
        );
    }
}

#[test]
fn local_event_and_query_collections_use_the_same_actor_policy() {
    let db = crate::db::Db::open(std::path::Path::new(":memory:")).unwrap();
    let channel =
        crate::chat::channels::state::create(&db, "fixture-owner", "Fixture channel").unwrap();
    for actor in ["fixture-owner", "fixture-member"] {
        let expected = crate::chat::events::channel_to_json(&channel, actor);
        let query = crate::chat::events::public_channels(&db, actor).unwrap();
        let event: serde_json::Value =
            serde_json::from_str(&crate::chat::events::channels_updated_payload(&db, actor))
                .unwrap();
        assert_eq!(query[0], expected);
        assert_eq!(event[0], expected);
    }
}
