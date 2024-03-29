#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum XpiGenericNodeSet<NID, TS> {
    /// Event is targeted at only one specific node.
    /// Any resources can be used from the node's vhL description.
    Unicast(NID),

    /// Event is targeted at only one node, but through traits interface.
    /// More expensive in terms of size and processing, but gives other benefits.
    UnicastTraits { destination: NID, traits: TS },

    /// Event is targeted at many nodes at once. Only nodes implementing a set of common traits can
    /// be addressed that way.
    ///
    /// Trait in this context is an xPI block defined and published to the Registry with a particular version.
    /// Might be thought of as an abstract class as well.
    ///
    /// Examples of xpi traits:
    /// * log - to e.g. subscribe to all node's logs at once
    /// * bootloader - to e.g. request all firmware versions
    /// * power_mgmt - to e.g. put all nodes to sleep
    /// Other more specific traits that only some nodes would implement:
    /// * led_feedback - to e.g. enable or disable led on devices
    /// * canbus_counters - to monitor CANBus status across the whole network
    Multicast {
        /// List of traits a node have to implement.
        /// Uri structure is arranged differently for this kind of requests.
        /// For example if 3 traits were provided, then there are /0, /1, /2 resources,
        /// each corresponding to the trait specified, in order.
        /// So e.g. it is possible to call 3 different functions from 3 different traits in one request.
        traits: TS,

        /// For preventing loops
        original_source: NID,
    },

    /// Event is targeted at all nodes at once.
    /// ver 0.2.0: use Multicast with traits = [Broadcast] and keep this Reserved?
    /// reserve e.g. trait gid = 0 for Broadcast, that way message stays the same in size.
    Broadcast {
        /// For preventing loops
        original_source: NID,
    },
}
