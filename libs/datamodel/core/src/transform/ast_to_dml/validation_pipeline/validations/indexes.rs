use datamodel_connector::ConnectorCapability;

use crate::ast::Span;
use crate::transform::ast_to_dml::db::IndexAlgorithm;
use crate::{
    common::preview_features::PreviewFeature,
    diagnostics::{DatamodelError, Diagnostics},
    transform::ast_to_dml::db::{walkers::IndexWalker, ConstraintName, ParserDatabase},
};

/// Different databases validate index and unique constraint names in a certain namespace.
/// Validates index and unique constraint names against the database requirements.
pub(crate) fn has_a_unique_constraint_name(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    let name = index.final_database_name();
    let model = index.model();

    for violation in db.scope_violations(model.model_id(), ConstraintName::Index(name.as_ref())) {
        let message = format!(
            "The given constraint name `{}` has to be unique in the following namespace: {}. Please provide a different name using the `map` argument.",
            name,
            violation.description(model.name()),
        );

        let span = index
            .ast_attribute()
            .map(|a| {
                let from_arg = a.span_for_argument("map").or_else(|| a.span_for_argument("name"));
                from_arg.unwrap_or(a.span)
            })
            .unwrap_or_else(|| model.ast_model().span);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            &message,
            index.attribute_name(),
            span,
        ));
    }
}

/// sort and length are not yet allowed
pub(crate) fn uses_length_or_sort_without_preview_flag(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if db.preview_features.contains(PreviewFeature::ExtendedIndexes) {
        return;
    }

    if index
        .scalar_field_attributes()
        .any(|f| f.sort_order().is_some() || f.length().is_some())
    {
        let message = "You must enable `extendedIndexes` preview feature to use sort or length parameters.";

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            message,
            index.attribute_name(),
            index.ast_attribute().map(|i| i.span).unwrap_or_else(Span::empty),
        ));
    }
}

/// The database must support the index length prefix for it to be allowed in the data model.
pub(crate) fn field_length_prefix_supported(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if db
        .active_connector()
        .has_capability(ConnectorCapability::IndexColumnLengthPrefixing)
    {
        return;
    }

    if index.scalar_field_attributes().any(|f| f.length().is_some()) {
        let message = "The length argument is not supported in an index definition with the current connector";
        let span = index.ast_attribute().map(|i| i.span).unwrap_or_else(Span::empty);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            message,
            index.attribute_name(),
            span,
        ));
    }
}

/// Is `Hash` supported as `type`
pub(crate) fn index_algorithm_is_supported(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if db
        .active_connector()
        .has_capability(ConnectorCapability::UsingHashIndex)
    {
        return;
    }

    if let Some(IndexAlgorithm::Hash) = index.attribute().algorithm {
        let message = "The given type argument is not supported with the current connector";
        let span = index
            .ast_attribute()
            .and_then(|i| i.span_for_argument("type"))
            .unwrap_or_else(Span::empty);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(message, "index", span));
    }
}

/// `@@fulltext` attribute is not available without `fullTextIndex` preview feature.
pub(crate) fn fulltext_index_preview_feature_enabled(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if db.preview_features.contains(PreviewFeature::FullTextIndex) {
        return;
    }

    if index.attribute().is_fulltext() {
        let message = "You must enable `fullTextIndex` preview feature to be able to define a @@fulltext index.";

        let span = index
            .ast_attribute()
            .map(|i| i.span)
            .unwrap_or_else(|| index.model().ast_model().span);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            message, "fulltext", span,
        ));
    }
}

/// `@@fulltext` should only be available if we support it in the database.
pub(crate) fn fulltext_index_supported(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if db.active_connector().has_capability(ConnectorCapability::FullTextIndex) {
        return;
    }

    if index.attribute().is_fulltext() {
        let message = "Defining fulltext indexes is not supported with the current connector.";

        let span = index
            .ast_attribute()
            .map(|i| i.span)
            .unwrap_or_else(|| index.model().ast_model().span);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            message, "fulltext", span,
        ));
    }
}

/// Defining the `type` must be with `extendedIndexes` preview feature.
pub(crate) fn index_algorithm_preview_feature(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if db.preview_features.contains(PreviewFeature::ExtendedIndexes) {
        return;
    }

    if index.attribute().algorithm.is_some() {
        let message = "You must enable `extendedIndexes` preview feature to be able to define the index type.";

        let span = index
            .ast_attribute()
            .and_then(|i| i.span_for_argument("type"))
            .unwrap_or_else(Span::empty);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(message, "index", span));
    }
}

/// `@@fulltext` index columns should not define `length` argument.
pub(crate) fn fulltext_columns_should_not_define_length(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if !db.preview_features.contains(PreviewFeature::FullTextIndex) {
        return;
    }

    if !db.active_connector().has_capability(ConnectorCapability::FullTextIndex) {
        return;
    }

    if !index.attribute().is_fulltext() {
        return;
    }

    if index.scalar_field_attributes().any(|f| f.length().is_some()) {
        let message = "The length argument is not supported in a @@fulltext attribute.";
        let span = index
            .ast_attribute()
            .map(|i| i.span)
            .unwrap_or_else(|| index.model().ast_model().span);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            message,
            index.attribute_name(),
            span,
        ));
    }
}

/// Only MongoDB supports sort order in a fulltext index.
pub(crate) fn fulltext_column_sort_is_supported(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if !db.preview_features.contains(PreviewFeature::FullTextIndex) {
        return;
    }

    if !db.active_connector().has_capability(ConnectorCapability::FullTextIndex) {
        return;
    }

    if !index.attribute().is_fulltext() {
        return;
    }

    if db
        .active_connector()
        .has_capability(ConnectorCapability::SortOrderInFullTextIndex)
    {
        return;
    }

    if index.scalar_field_attributes().any(|f| f.sort_order().is_some()) {
        let message = "The sort argument is not supported in a @@fulltext attribute in the current connector.";
        let span = index
            .ast_attribute()
            .map(|i| i.span)
            .unwrap_or_else(|| index.model().ast_model().span);

        diagnostics.push_error(DatamodelError::new_attribute_validation_error(
            message,
            index.attribute_name(),
            span,
        ));
    }
}

/// Mongo wants all text keys to be bundled together, so e.g. this doesn't work:
///
/// ```ignore
/// @@fulltext([a(sort: Asc), b, c(sort: Asc), d])
/// ```
pub(crate) fn fulltext_text_columns_should_be_bundled_together(
    db: &ParserDatabase<'_>,
    index: IndexWalker<'_, '_>,
    diagnostics: &mut Diagnostics,
) {
    if !db.preview_features.contains(PreviewFeature::FullTextIndex) {
        return;
    }

    if !db.active_connector().has_capability(ConnectorCapability::FullTextIndex) {
        return;
    }

    if !index.attribute().is_fulltext() {
        return;
    }

    if !db
        .active_connector()
        .has_capability(ConnectorCapability::SortOrderInFullTextIndex)
    {
        return;
    }

    enum State {
        // The empty state in the beginning. Must move to another state in every case.
        Init,
        // We've only had sorted fields so far.
        SortParamHead,
        // The bundle of text fields, we can have only one per index.
        TextFieldBundle,
        // The sort params after one text bundle.
        SortParamTail,
    }

    let mut state = State::Init;

    for field in index.scalar_field_attributes() {
        state = match state {
            State::Init if field.sort_order().is_some() => State::SortParamHead,
            State::SortParamHead if field.sort_order().is_some() => State::SortParamHead,
            State::TextFieldBundle if field.sort_order().is_some() => State::SortParamTail,
            State::SortParamTail if field.sort_order().is_some() => State::SortParamTail,
            State::Init | State::SortParamHead | State::TextFieldBundle => State::TextFieldBundle,
            State::SortParamTail => {
                let message = "All index fields must be listed adjacently in the fields argument.";

                let span = index
                    .ast_attribute()
                    .map(|i| i.span)
                    .unwrap_or_else(|| index.model().ast_model().span);

                diagnostics.push_error(DatamodelError::new_attribute_validation_error(
                    message, "fulltext", span,
                ));

                return;
            }
        }
    }
}

//TODO(extended indices) add db specific validations to sort and length