use crate::allowlist::Allowlist;
use crate::types::*;

pub fn decide(
    mode: Mode,
    event: &CookieEvent,
    bucket: CookieBucket,
    allowlist: &Allowlist,
) -> Decision {
    match mode {
        Mode::Observe => Decision {
            kind: DecisionKind::Allow,
            reason: "observe".into(),
        },
        Mode::CookieMonster => match event.action {
            CookieAction::Set => Decision {
                kind: DecisionKind::Deny,
                reason: "cookie_monster".into(),
            },
            CookieAction::Send | CookieAction::Js => Decision {
                kind: DecisionKind::Strip,
                reason: "cookie_monster".into(),
            },
        },
        Mode::Moderate => moderate_decide(event, bucket, allowlist),
    }
}

fn moderate_decide(event: &CookieEvent, bucket: CookieBucket, allowlist: &Allowlist) -> Decision {
    match allowlist.decision_for(&event.host, bucket) {
        Some(true) => Decision {
            kind: DecisionKind::Allow,
            reason: "allowlist".into(),
        },
        Some(false) => Decision {
            kind: DecisionKind::Deny,
            reason: "allowlist_type_blocked".into(),
        },
        None => match bucket {
            CookieBucket::Necessary => Decision {
                kind: DecisionKind::Allow,
                reason: "moderate_necessary".into(),
            },
            CookieBucket::Preferences if event.party == CookieParty::First => Decision {
                kind: DecisionKind::Allow,
                reason: "moderate_preferences".into(),
            },
            _ => Decision {
                kind: DecisionKind::Deny,
                reason: "moderate_blocked".into(),
            },
        },
    }
}
