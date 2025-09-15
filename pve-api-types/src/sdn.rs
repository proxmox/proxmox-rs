use crate::{SdnController, SdnVnet, SdnZone};

impl SdnVnet {
    /// returns the tag from the pending property if it has a value, otherwise it returns self.tag
    pub fn tag_pending(&self) -> Option<u32> {
        self.pending
            .as_ref()
            .and_then(|pending| pending.tag)
            .or(self.tag)
    }

    /// returns the zone from the pending property if it has a value, otherwise it returns
    /// self.zone
    pub fn zone_pending(&self) -> String {
        self.pending
            .as_ref()
            .and_then(|pending| pending.zone.clone())
            .or_else(|| self.zone.clone())
            .expect("zone must be set in either pending or root")
    }
}

impl SdnZone {}

impl SdnController {
    /// returns the ASN from the pending property if it has a value, otherwise it returns self.asn
    pub fn asn_pending(&self) -> Option<u32> {
        self.pending
            .as_ref()
            .and_then(|pending| pending.asn)
            .or(self.asn)
    }
}
