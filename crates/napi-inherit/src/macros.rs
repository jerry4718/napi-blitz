//! Ergonomic assembly of a [`LayerChain`] in reversed (parent-first) order.

/// Assemble a `LayerChain` in reversed order: the first expression is the
/// deepest parent, the last is the outermost own layer. The type of the
/// result is inferred at the use site (`new_from_chain` fixes it).
///
/// The accumulator starts at `()` (the `RootLayer` chain) and each
/// expression becomes the `own` of a nested `LayerChain`:
/// ```ignore
/// layer_chain!(
///     BaseLayer { base_value: 1 },
///     MidLayer { mid_value: 2 },
/// )
/// ```
/// expands to
/// ```ignore
/// LayerChain {
///     own: MidLayer { mid_value: 2 },
///     parent: LayerChain { own: BaseLayer { base_value: 1 }, parent: () },
/// }
/// ```
///
/// Prefix `..` to start from an already-built chain instead of `()`.
#[macro_export]
macro_rules! layer_chain {
    (.. $base:expr, $($own:expr),+ $(,)?) => {{
        let __layer_chain_acc = $base;
        $( let __layer_chain_acc = $crate::layer::LayerChain { own: $own, parent: __layer_chain_acc }; )+
        __layer_chain_acc
    }};
    ($($own:expr),+ $(,)?) => {{
        let __layer_chain_acc = ();
        $( let __layer_chain_acc = $crate::layer::LayerChain { own: $own, parent: __layer_chain_acc }; )+
        __layer_chain_acc
    }};
}

/// Build an instance from a data chain: `from_chain!((LeafLayer, env) ...)`
/// calls [`new_from_chain`](crate::class::new_from_chain) with a
/// `layer_chain!` assembled from the trailing expressions.
#[macro_export]
macro_rules! from_chain {
    (($for:ty, $env:expr) $($tt:tt)+) => {
        $crate::class::new_from_chain::<$for>($env, $crate::layer_chain!($($tt)+))
    };
}
