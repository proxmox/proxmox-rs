#![recursion_limit = "256"]

extern crate proc_macro;
extern crate proc_macro2;

use proc_macro::TokenStream;

#[macro_use]
mod error;

mod api_def;
mod parsing;
mod util;

mod api_macro;
mod router_macro;

fn handle_error(kind: &'static str, err: failure::Error) -> TokenStream {
    match err.downcast::<error::CompileError>() {
        Ok(err) => err.tokens,
        Err(err) => panic!("error in {}: {}", kind, err),
    }
}

/// This is the `#[api(api definition)]` attribute for functions. An Api definition defines the
/// parameters and return type of an API call. The function will automatically be wrapped in a
/// function taking and returning a json `Value`, while performing validity checks on both input
/// and output.
///
/// Example:
/// ```ignore
/// #[api({
///     parameters: {
///         // Short form: [`optional`] TYPE ("description")
///         name: string ("A person's name"),
///         gender: optional string ("A person's gender"),
///         // Long form uses json-ish syntax:
///         coolness: {
///             type: integer, // we don't enclose type names in quotes though...
///             description: "the coolness of a person, using the coolness scale",
///             minimum: 0,
///             maximum: 10,
///         },
///         // Hyphenated parameters are allowed, but need quotes (due to how proc_macro
///         // TokenStreams work)
///         "is-weird": optional float ("hyphenated names must be enclosed in quotes")
///     },
///     // TODO: returns: {}
/// })]
/// fn test() {
/// }
/// ```
#[proc_macro_attribute]
pub fn api(attr: TokenStream, item: TokenStream) -> TokenStream {
    match api_macro::api_macro(attr.into(), item.into()) {
        Ok(output) => output.into(),
        Err(err) => handle_error("api definition", err),
    }
}

/// The router macro helps to avoid having to type out strangely nested `Router` expressions.
///
/// Note that without `proc_macro_hack` we currently cannot use macros in expression position, so
/// this cannot be used inline within an expression.
///
/// Example:
/// ```ignore
/// router!{
///     let my_router = {
///         /people/{person}: {
///             POST: create_person,
///             GET: get_person,
///             PUT: update_person,
///             DELETE: delete_person,
///         },
///         /people/{person}/kick: { POST: kick_person },
///         /groups/{group}: {
///             /: {
///                 POST: create_group,
///                 PUT: update_group_info,
///                 GET: get_group_info,
///                 DELETE: delete_group,
///             },
///             /people/{person}: {
///                 POST: add_person_to_group,
///                 DELETE: delete_person_from_group,
///                 PUT: update_person_details_for_group,
///                 GET: get_person_details_from_group,
///             },
///         },
///         /other: (an_external_router)
///     };
/// }
/// ```
///
/// The above should produce the following output:
/// ```ignore
/// let my_router = Router::new()
///     .subdir(
///         "people",
///         Router::new()
///             .parameter_subdir(
///                 "person",
///                 Router::new()
///                     .post(create_person)
///                     .get(get_person)
///                     .put(update_person)
///                     .delete(delete_person)
///                     .subdir(
///                         "kick",
///                         Router::new()
///                             .post(kick_person)
///                     )
///             )
///     )
///     .subdir(
///         "groups",
///         Router::new()
///             .parameter_subdir(
///                 "group",
///                 Router::new()
///                     .post(create_group)
///                     .put(update_group_info)
///                     .get(get_group_info)
///                     .delete(delete_group_info)
///                     .subdir(
///                         "people",
///                         Router::new()
///                             .parameter_subdir(
///                                 "person",
///                                 Router::new()
///                                     .post(add_person_to_group)
///                                     .delete(delete_person_from_group)
///                                     .put(update_person_details_for_group)
///                                     .get(get_person_details_from_group)
///                             )
///                     )
///             )
///     )
///     .subdir("other", an_external_router)
///     ;
/// ```
#[proc_macro]
pub fn router(input: TokenStream) -> TokenStream {
    // TODO...
    match router_macro::router_macro(input.into()) {
        Ok(output) => output.into(),
        Err(err) => handle_error("router", err),
    }
}
