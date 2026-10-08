//! Stable diagnostics translation keys mapped to the existing catalog.
//! This host-only bridge does not introduce new copy or infer provider prices.

use codewhale_localization::MessageId;

pub(super) fn resolve(key: &str) -> Option<MessageId> {
    if let Some(reason) = key.strip_prefix("cost_reason_") {
        return Some(crate::pricing::UnpricedReason::from_label(reason).message_id());
    }
    Some(match key {
        "cmd_change_header" => MessageId::CmdChangeHeader,
        "cmd_change_previous_version" => MessageId::CmdChangePreviousVersion,
        "cmd_change_translation_unavailable" => MessageId::CmdChangeTranslationUnavailable,
        "cmd_change_translation_queued" => MessageId::CmdChangeTranslationQueued,
        "cmd_tokens_not_reported" => MessageId::CmdTokensNotReported,
        "cmd_tokens_context_with_window" => MessageId::CmdTokensContextWithWindow,
        "cmd_tokens_cache_both" => MessageId::CmdTokensCacheBoth,
        "cmd_tokens_cache_hit_only" => MessageId::CmdTokensCacheHitOnly,
        "cmd_tokens_cache_miss_only" => MessageId::CmdTokensCacheMissOnly,
        "cmd_tokens_report" => MessageId::CmdTokensReport,
        "cmd_tokens_cache_write_total" => MessageId::CmdTokensCacheWriteTotal,
        "cmd_cost_report" => MessageId::CmdCostReport,
        "cmd_cost_report_subtotal" => MessageId::CmdCostReportSubtotal,
        "cmd_cost_report_unknown" => MessageId::CmdCostReportUnknown,
        "cmd_cost_unknown_value" => MessageId::CmdCostUnknownValue,
        "cmd_cost_estimate_only" => MessageId::CmdCostEstimateOnly,
        "cmd_cost_coverage_unknown_legacy" => MessageId::CmdCostCoverageUnknownLegacy,
        "cmd_cost_coverage" => MessageId::CmdCostCoverage,
        "cmd_cost_unpriced_turns" => MessageId::CmdCostUnpricedTurns,
        "cmd_cost_unpriced_classes" => MessageId::CmdCostUnpricedClasses,
        "cmd_cost_pricing_provenance" => MessageId::CmdCostPricingProvenance,
        "cmd_cost_live_pricing_downgraded" => MessageId::CmdCostLivePricingDowngraded,
        "cmd_cost_live_pricing_unavailable" => MessageId::CmdCostLivePricingUnavailable,
        "cmd_cost_routes_header" => MessageId::CmdCostRoutesHeader,
        "cmd_cache_header" => MessageId::CmdCacheHeader,
        "cmd_cache_no_data" => MessageId::CmdCacheNoData,
        "cmd_cache_totals" => MessageId::CmdCacheTotals,
        "cmd_cache_unpriced_note" => MessageId::CmdCacheUnpricedNote,
        "cmd_cache_advice" => MessageId::CmdCacheAdvice,
        "cmd_cache_footnote" => MessageId::CmdCacheFootnote,
        "cmd_cache_rate_parent" => MessageId::CmdCacheRateParent,
        "cmd_cache_rate_agents" => MessageId::CmdCacheRateAgents,
        "cmd_cache_rate_combined" => MessageId::CmdCacheRateCombined,
        "cmd_cache_session_rates" => MessageId::CmdCacheSessionRates,
        _ => return None,
    })
}
