/** Computed styles are bounded shared data consumed by native layout, painting and control guards. */
use {
    super::flex_model::*,
    super::formatting::{ColorScheme, LineHeight},
    crate::document::Appearance,
    serde::{Deserialize, Serialize},
};
/// Required resources retain their document order before acquisition.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum StyleSource {
    Inline(String),
    Linked(String),
}
/// One computed style is shared by rendering, native widgets and action guards.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ComputedStyle {
    pub hidden: bool,
    pub interactive: bool,
    pub inline: bool,
    pub font_size: f64,
    pub bold: bool,
    pub color: Option<String>,
    pub background: Option<String>,
    pub margin: [f64; 4],
    /// Automatic margins resolve only after the containing block and border box are known.
    pub margin_auto: [bool; 4],
    /// Inherited factors remain relative to each descendant's own font.
    pub line_height: LineHeight,
    /// Offered schemes retain the author's choice independently of the host preference.
    pub color_scheme: ColorScheme,
    /// Light-dark colors use this resolved authored scheme.
    pub used_scheme: Appearance,
    pub padding: [f64; 4],
    pub width: Option<f64>,
    pub percent_width: bool,
    pub max_width: Option<f64>,
    pub border: f64,
    pub border_color: Option<String>,
    pub align: String,
    /// Authored dimensions name either content or the complete painted border box.
    pub box_sizing: BoxSizing,
    /// Flex participation is explicit rather than inferred from child control types.
    pub display_flex: bool,
    /// The main axis determines which authored basis can be definite.
    pub flex_direction: FlexDirection,
    /// Wrapped rows form lines without introducing unsupported column heights.
    pub flex_wrap: FlexWrap,
    /// Automatic, pixel and percentage bases retain distinct layout meaning.
    pub flex_basis: FlexBasis,
    /// Nonnegative growth distributes only real positive main-axis free space.
    pub flex_grow: f64,
    /// Shrinkage is weighted by the item's base content size during allocation.
    pub flex_shrink: f64,
    /// Row separation remains independent of column separation when wrapping.
    pub row_gap: f64,
    /// Only between-item separation contributes to the line's horizontal budget.
    pub column_gap: f64,
    /// Main-axis alignment consumes only remaining nonnegative free space.
    pub justify_content: JustifyContent,
    /// Cross-axis alignment controls the allocated item subtree as one unit.
    pub align_items: AlignItems,
    /// None retains the content minimum; explicit zero permits supported shrinking.
    pub min_width: Option<f64>,
}
impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            hidden: false,
            interactive: true,
            inline: false,
            font_size: crate::constants::CSS_ROOT_FONT_SIZE,
            bold: false,
            color: None,
            background: None,
            margin: [0.0; 4],
            margin_auto: [false; 4],
            line_height: LineHeight::Normal,
            color_scheme: ColorScheme::Normal,
            used_scheme: Appearance::Light,
            padding: [0.0; 4],
            width: None,
            percent_width: false,
            max_width: None,
            border: 0.0,
            border_color: None,
            align: "left".into(),
            box_sizing: BoxSizing::ContentBox,
            display_flex: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_basis: FlexBasis::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            row_gap: 0.0,
            column_gap: 0.0,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            min_width: None,
        }
    }
}
