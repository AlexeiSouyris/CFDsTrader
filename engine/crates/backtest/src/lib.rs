//! Backtest (Fase 3): salidas de una operación (spec §8 y §12), costos (§11) y el recorrido
//! completo de un instrumento con su registro de operaciones y comprar y mantener.

pub mod config;
pub mod costs;
pub mod evaluation;
pub mod exits;
pub mod export;
pub mod metrics;
pub mod portfolio;
pub mod proposals;
pub mod rates;
pub mod rollover;
pub mod runner;
pub mod scenario;
pub mod sensitivity;
pub mod setup;
pub mod variants;

pub use config::{BacktestConfig, CostConfig, ExitConfig, MetricsConfig};
pub use costs::{
    BasisFlow, Commission, Execution, InstrumentCosts, commission, commodity_basis,
    commodity_funding, fill_price, funding, funding_cutoffs, funding_nights, funding_over,
};
pub use evaluation::{
    Aggregate, EvalError, EvaluationConfig, FoldResult, Inputs, PeriodResult, Slice, aggregate,
    evaluate_holdout, evaluate_period, evaluate_period_with, research_period, walk_forward,
};
pub use exits::{ExitReason, Fill, Outcome, Trade, simulate};
pub use export::{ChartExport, chart_export};
pub use metrics::{Metrics, evaluate};
pub use portfolio::{
    KillSwitchEvent, KillSwitchEventKind, Market, PortfolioError, PortfolioRejected,
    PortfolioReport, PortfolioTrade, run_portfolio,
};
pub use proposals::{
    Label, Overlap, Proposal, ProposalReport, ProposalRules, Tally, proposals, tally_by,
};
pub use rates::{AccountFx, DailyRates, FundingRates, FxRates};
pub use rollover::mark_rollovers;
pub use runner::{
    BuyAndHold, CloseReason, ExitRecord, RejectedSignal, Report, RunError, Setup, TradeRecord,
    buy_and_hold, choose, execute, pick, run, run_checked, run_with_news,
};
pub use scenario::{
    AssumedTerms, SCENARIO_SOURCE, StudyAssumptions, add_weekdays, apply_assumptions,
    apply_funding_scenario, rates_from_csv,
};
pub use sensitivity::{
    ConfigTexts, ParamFile, SensitivityParam, SensitivityPoint, Stability, override_toml,
    sensitivity, setup_with, stability, value_text,
};
pub use setup::{MarketTerms, SetupError, market_terms_from_catalog, setup_from_catalog};
pub use variants::{
    Override, TradeStats, Variant, entry_filter, research_blocks, setup_for, variants_from_toml,
};
