/** Static table validation derives bounded row/cell relationships from source order. */
use super::{Document, Node, TableCell};
use crate::{
    constants::{TABLE_CELLS, TABLE_COLUMNS, TABLE_ROWS},
    protocol::{EngineError, EngineResult},
};

/** Only ordinary nonnested table structure enters the native grid layout. */
pub(super) fn annotate(document: &mut Document) -> EngineResult<()> {
    validate_tree(document)?;
    let tables = ids(document, "table");
    for table in tables {
        annotate_table(document, table)?;
    }
    Ok(())
}

/** Malformed relationships and spanning models fail before candidate publication. */
fn validate_tree(document: &Document) -> EngineResult<()> {
    for node in &document.nodes {
        if table_tag(&node.tag) {
            validate_parent(document, node)?;
        }
        if node.tag == "table" && ancestor(document, node, "table") {
            return Err(unsupported());
        }
        validate_spans(node)?;
    }
    for table in ids(document, "table") {
        validate_children(
            document,
            table,
            &["caption", "thead", "tbody", "tfoot", "tr"],
        )?;
    }
    for group in ids(document, "thead")
        .into_iter()
        .chain(ids(document, "tbody"))
        .chain(ids(document, "tfoot"))
    {
        validate_children(document, group, &["tr"])?;
    }
    for row in ids(document, "tr") {
        validate_children(document, row, &["th", "td"])?;
    }
    Ok(())
}

/** Structural containers reject markup that the grid would otherwise silently omit. */
fn validate_children(document: &Document, parent: usize, allowed: &[&str]) -> EngineResult<()> {
    if document
        .nodes
        .iter()
        .any(|node| node.parent == Some(parent) && !allowed.contains(&node.tag.as_str()))
    {
        return Err(unsupported());
    }
    Ok(())
}

/** Table children must use the supported direct structural parents. */
fn validate_parent(document: &Document, node: &Node) -> EngineResult<()> {
    let parent = node
        .parent
        .and_then(|id| document.nodes.get(id))
        .map(|parent| parent.tag.as_str());
    let valid = match node.tag.as_str() {
        "table" => !ancestor(document, node, "table"),
        "caption" | "thead" | "tbody" | "tfoot" => parent == Some("table"),
        "tr" => [Some("table"), Some("thead"), Some("tbody"), Some("tfoot")].contains(&parent),
        "th" | "td" => parent == Some("tr"),
        _ => true,
    };
    if valid { Ok(()) } else { Err(unsupported()) }
}

/** Spans and unsupported header scopes cannot desynchronize grid geometry. */
fn validate_spans(node: &Node) -> EngineResult<()> {
    if !["th", "td"].contains(&node.tag.as_str()) {
        return Ok(());
    }
    for name in ["rowspan", "colspan"] {
        if node.attributes.get(name).is_some_and(|value| value != "1") {
            return Err(unsupported());
        }
    }
    if node.tag == "th"
        && node
            .attributes
            .get("scope")
            .is_some_and(|scope| !["row", "col"].contains(&scope.as_str()))
    {
        return Err(unsupported());
    }
    Ok(())
}

/** Each table has independently bounded source-ordered rows and columns. */
fn annotate_table(document: &mut Document, table: usize) -> EngineResult<()> {
    validate_groups(document, table)?;
    let rows = ids(document, "tr")
        .into_iter()
        .filter(|row| closest_table(document, *row) == Some(table))
        .collect::<Vec<_>>();
    if rows.len() > TABLE_ROWS {
        return Err(EngineError::limit("table rows", TABLE_ROWS));
    }
    if rows
        .iter()
        .map(|row| children(document, *row).len())
        .sum::<usize>()
        > TABLE_CELLS
    {
        return Err(EngineError::limit("table cells", TABLE_CELLS));
    }
    annotate_cells(document, table, &rows)
}

/** Duplicate caption and header/footer groups make source associations ambiguous. */
fn validate_groups(document: &Document, table: usize) -> EngineResult<()> {
    for tag in ["caption", "thead", "tfoot"] {
        if document
            .nodes
            .iter()
            .filter(|node| node.parent == Some(table) && node.tag == tag)
            .count()
            > 1
        {
            return Err(unsupported());
        }
    }
    Ok(())
}

/** Header associations use explicit row/column scope or the first header row. */
fn annotate_cells(document: &mut Document, table: usize, rows: &[usize]) -> EngineResult<()> {
    let mut column_headers = Vec::new();
    let first_header_row = rows.iter().position(|row| {
        let cells = children(document, *row);
        !cells.is_empty()
            && (closest_tag(document, *row, "thead", table)
                || cells.iter().all(|id| document.nodes[*id].tag == "th"))
    });
    for (row_index, row) in rows.iter().enumerate() {
        let row_cells = children(document, *row);
        if row_cells.len() > TABLE_COLUMNS {
            return Err(EngineError::limit("table columns", TABLE_COLUMNS));
        }
        for (column_index, id) in row_cells.iter().enumerate() {
            if is_column_header(document, *id, row_index, table, first_header_row) {
                column_headers.push((*id, column_index));
            }
        }
    }
    for (row_index, row) in rows.iter().enumerate() {
        let row_cells = children(document, *row);
        let row_headers = row_cells
            .iter()
            .copied()
            .filter(|id| {
                document.nodes[*id].tag == "th"
                    && document.nodes[*id]
                        .attributes
                        .get("scope")
                        .is_some_and(|scope| scope == "row")
            })
            .collect::<Vec<_>>();
        for (column_index, id) in row_cells.iter().enumerate() {
            let headers = headers_for(document, *id, column_index, &column_headers, &row_headers);
            document.nodes[*id].table_cell = Some(TableCell {
                table,
                row: *row,
                row_index: row_index + 1,
                column_index: column_index + 1,
                headers,
            });
        }
    }
    Ok(())
}

/** Column headers apply down their column; explicit row headers apply across the row. */
fn headers_for(
    document: &Document,
    cell: usize,
    column: usize,
    columns: &[(usize, usize)],
    rows: &[usize],
) -> Vec<usize> {
    let mut result = columns
        .iter()
        .filter(|(id, index)| *index == column && *id != cell)
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    result.extend(rows.iter().copied().filter(|id| *id != cell));
    if document.nodes[cell].tag == "th" {
        result.retain(|id| *id != cell);
    }
    result
}

/** A header row is recognized by the enclosing thead or inferred from first-row th cells. */
fn is_column_header(
    document: &Document,
    id: usize,
    row: usize,
    table: usize,
    first: Option<usize>,
) -> bool {
    let node = &document.nodes[id];
    node.tag == "th"
        && match node.attributes.get("scope").map(String::as_str) {
            Some("col") => true,
            Some("row") => false,
            None => first == Some(row) || closest_tag(document, id, "thead", table),
            _ => false,
        }
}

/** Group membership is bounded to the nearest containing table. */
fn closest_tag(document: &Document, row: usize, tag: &str, table: usize) -> bool {
    let mut parent = document.nodes[row].parent;
    while let Some(id) = parent {
        if id == table {
            return false;
        }
        if document.nodes[id].tag == tag {
            return true;
        }
        parent = document.nodes[id].parent;
    }
    false
}

/** Table identity for a row follows parent links, never page-controlled HTML IDs. */
fn closest_table(document: &Document, node: usize) -> Option<usize> {
    let mut parent = document.nodes[node].parent;
    while let Some(id) = parent {
        if document.nodes[id].tag == "table" {
            return Some(id);
        }
        parent = document.nodes[id].parent;
    }
    None
}

/** Parent ancestry checks do not recurse through untrusted markup depth. */
fn ancestor(document: &Document, node: &Node, tag: &str) -> bool {
    let mut parent = node.parent;
    while let Some(id) = parent {
        if document.nodes[id].tag == tag {
            return true;
        }
        parent = document.nodes[id].parent;
    }
    false
}

/** Direct children preserve source order and stable local node IDs. */
fn children(document: &Document, parent: usize) -> Vec<usize> {
    document
        .nodes
        .iter()
        .filter(|node| node.parent == Some(parent) && ["td", "th"].contains(&node.tag.as_str()))
        .map(|node| node.local_id)
        .collect()
}

/** Source traversal retains a stable bounded identity vector. */
fn ids(document: &Document, tag: &str) -> Vec<usize> {
    document
        .nodes
        .iter()
        .filter(|node| node.tag == tag)
        .map(|node| node.local_id)
        .collect()
}

/** Structural tags are validated even when author markup placed them outside a table. */
fn table_tag(tag: &str) -> bool {
    [
        "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
    ]
    .contains(&tag)
}

/** Unsupported table behavior is explicit and leaves the committed page intact. */
fn unsupported() -> EngineError {
    EngineError::unsupported(
        "This table structure is malformed or outside the supported static subset.",
    )
}
