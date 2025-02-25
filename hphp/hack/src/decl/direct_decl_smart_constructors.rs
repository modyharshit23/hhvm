/**
 * Copyright (c) 2019, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
*/
use parser_rust as parser;

use escaper::{extract_unquoted_string, unescape_double, unescape_single};
use flatten_smart_constructors::{FlattenOp, FlattenSmartConstructors};
use hhbc_rust::string_utils::GetName;
use oxidized::{
    aast,
    ast_defs::Id,
    direct_decl_parser::Decls,
    pos::Pos,
    s_map::SMap,
    typing_defs::{Ty, Ty_},
    typing_reason::Reason,
};
use parser::{
    indexed_source_text::IndexedSourceText, lexable_token::LexableToken, token_kind::TokenKind,
};

pub use crate::direct_decl_smart_constructors_generated::*;

pub fn empty_decls() -> Decls {
    Decls {
        classes: SMap::new(),
        funs: SMap::new(),
        typedefs: SMap::new(),
        consts: SMap::new(),
    }
}

fn try_collect<T, E>(vec: Vec<Result<T, E>>) -> Result<Vec<T>, E> {
    vec.into_iter().try_fold(Vec::new(), |mut acc, elem| {
        acc.push(elem?);
        Ok(acc)
    })
}

fn mangle_xhp_id(mut name: String) -> String {
    fn ignore_id(name: &str) -> bool {
        name.starts_with("class@anonymous") || name.starts_with("Closure$")
    }

    fn is_xhp(name: &str) -> bool {
        name.chars().next().map_or(false, |c| c == ':')
    }

    if !ignore_id(&name) {
        if is_xhp(&name) {
            name.replace_range(..1, "xhp_")
        }
        name.replace(":", "__").replace("-", "_")
    } else {
        name
    }
}

pub fn get_name(namespace: &str, name: &Node_) -> Result<String, String> {
    fn qualified_name_from_parts(namespace: &str, parts: &Vec<Node_>) -> Result<String, String> {
        let mut qualified_name = String::new();
        let mut leading_backslash = false;
        for (index, part) in parts.into_iter().enumerate() {
            match part {
                Node_::Name(name) => {
                    qualified_name.push_str(&String::from_utf8_lossy(name.get().as_slice()))
                }
                Node_::Backslash if index == 0 => leading_backslash = true,
                Node_::ListItem(listitem) => {
                    if let (Node_::Name(name), Node_::Backslash) = &**listitem {
                        qualified_name.push_str(&String::from_utf8_lossy(name.get().as_slice()));
                        qualified_name.push_str("\\");
                    } else {
                        return Err(format!(
                            "Expected a name or backslash, but got {:?}",
                            listitem
                        ));
                    }
                }
                n => {
                    return Err(format!(
                        "Expected a name, backslash, or list item, but got {:?}",
                        n
                    ))
                }
            }
        }
        Ok(if leading_backslash || namespace.is_empty() {
            qualified_name // globally qualified name
        } else {
            namespace.to_owned() + "\\" + &qualified_name
        })
    }

    match name {
        Node_::Name(name) => {
            // always a simple name
            let name = name.to_string();
            Ok(if namespace.is_empty() {
                name
            } else {
                namespace.to_owned() + "\\" + &name
            })
        }
        Node_::XhpName(name) => {
            // xhp names are always unqualified
            let name = name.to_string();
            Ok(mangle_xhp_id(name))
        }
        Node_::QualifiedName(parts) => qualified_name_from_parts(namespace, &parts),
        n => {
            return Err(format!(
                "Expected a name, XHP name, or qualified name, but got {:?}",
                n
            ))
        }
    }
}

#[derive(Clone, Debug)]
pub struct State<'a> {
    pub source_text: IndexedSourceText<'a>,
    pub decls: Decls,
}

#[derive(Clone, Debug)]
pub enum HintValue {
    String,
    Int,
    Float,
    Num,
    Bool,
    Apply(GetName),
}

#[derive(Clone, Debug)]
pub enum Node_ {
    List(Vec<Node_>),
    Ignored,
    // tokens
    Name(GetName),
    String(GetName),
    XhpName(GetName),
    Hint(HintValue, Pos),
    Backslash,
    ListItem(Box<(Node_, Node_)>),
    Class,
    Interface,
    Trait,
    Extends,
    Implements,
    Abstract,
    Final,
    Static,
    QualifiedName(Vec<Node_>),
    ScopeResolutionExpression(Box<(Node_, Node_)>),
    // declarations
    ClassDecl(Box<ClassDeclChildren>),
    FunctionDecl(Box<Node_>),
    MethodDecl(Box<Node_>),
    EnumDecl(Box<EnumDeclChildren>),
    TraitUseClause(Box<Node_>),
    RequireExtendsClause(Box<Node_>),
    RequireImplementsClause(Box<Node_>),
    Define(Box<Node_>),
    TypeAliasDecl(Box<TypeAliasDeclChildren>),
    NamespaceDecl(Box<Node_>, Box<Node_>),
    EmptyBody,
}

pub type Node = Result<Node_, String>;

#[derive(Clone, Debug)]
pub struct ClassDeclChildren {
    pub modifiers: Node_,
    pub kind: Node_,
    pub name: Node_,
    pub attributes: Node_,
    pub extends: Node_,
    pub implements: Node_,
    pub constrs: Node_,
    pub body: Node_,
}

#[derive(Clone, Debug)]
pub struct EnumDeclChildren {
    pub name: Node_,
    pub attributes: Node_,
}

#[derive(Clone, Debug)]
pub struct TypeAliasDeclChildren {
    pub name: Node_,
    pub attributes: Node_,
}

impl<'a> FlattenOp for DirectDeclSmartConstructors<'_> {
    type S = Node;

    fn flatten(lst: Vec<Self::S>) -> Self::S {
        let r = lst
            .into_iter()
            .map(|s| match s {
                Ok(Node_::List(children)) => children.into_iter().map(|x| Ok(x)).collect(),
                x => {
                    if Self::is_zero(&x) {
                        vec![]
                    } else {
                        vec![x]
                    }
                }
            })
            .flatten()
            .collect::<Vec<Self::S>>();
        let mut r = try_collect(r)?;
        Ok(match r.as_slice() {
            [] => Node_::Ignored,
            [_] => r.pop().unwrap(),
            _ => Node_::List(r),
        })
    }

    fn zero() -> Self::S {
        Ok(Node_::Ignored)
    }

    fn is_zero(s: &Self::S) -> bool {
        if let Ok(s) = s {
            match s {
                Node_::Ignored |
                // tokens
                Node_::Name(_) |
                Node_::String(_) |
                Node_::XhpName(_) |
                Node_::Hint(_, _) |
                Node_::Backslash |
                Node_::ListItem(_) |
                Node_::Class |
                Node_::Interface |
                Node_::Trait |
                Node_::Extends |
                Node_::Implements |
                Node_::Abstract |
                Node_::Final |
                Node_::Static |
                Node_::QualifiedName(_) => true,
                _ => false,
            }
        } else {
            false
        }
    }
}

impl<'a> FlattenSmartConstructors<'a, State<'a>> for DirectDeclSmartConstructors<'a> {
    fn make_token(&mut self, token: Self::Token) -> Self::R {
        let token_text = || {
            self.state
                .source_text
                .source_text()
                .sub(
                    token.leading_start_offset().unwrap_or(0) + token.leading_width(),
                    token.width(),
                )
                .to_vec()
        };
        let token_pos = || {
            self.state
                .source_text
                .relative_pos(token.start_offset(), token.end_offset() + 1)
        };
        let kind = token.kind();
        Ok(match kind {
            TokenKind::Name => Node_::Name(GetName::new(token_text(), |string| string)),
            TokenKind::DecimalLiteral => Node_::String(GetName::new(token_text(), |string| string)),
            TokenKind::SingleQuotedStringLiteral => {
                Node_::String(GetName::new(token_text(), |string| {
                    let tmp = unescape_single(string.as_str()).ok().unwrap();
                    extract_unquoted_string(&tmp, 0, tmp.len()).ok().unwrap()
                }))
            }
            TokenKind::DoubleQuotedStringLiteral => {
                Node_::String(GetName::new(token_text(), |string| {
                    let tmp = unescape_double(string.as_str()).ok().unwrap();
                    extract_unquoted_string(&tmp, 0, tmp.len()).ok().unwrap()
                }))
            }
            TokenKind::XHPClassName => Node_::XhpName(GetName::new(token_text(), |string| string)),
            TokenKind::String => Node_::Hint(HintValue::String, token_pos()),
            TokenKind::Int => Node_::Hint(HintValue::Int, token_pos()),
            TokenKind::Float => Node_::Hint(HintValue::Float, token_pos()),
            TokenKind::Double => Node_::Hint(
                HintValue::Apply(GetName::new(token_text(), |string| string)),
                token_pos(),
            ),
            TokenKind::Num => Node_::Hint(HintValue::Num, token_pos()),
            TokenKind::Bool => Node_::Hint(HintValue::Bool, token_pos()),
            TokenKind::Boolean => Node_::Hint(
                HintValue::Apply(GetName::new(token_text(), |string| string)),
                token_pos(),
            ),
            TokenKind::Backslash => Node_::Backslash,
            TokenKind::Class => Node_::Class,
            TokenKind::Trait => Node_::Trait,
            TokenKind::Interface => Node_::Interface,
            TokenKind::Extends => Node_::Extends,
            TokenKind::Implements => Node_::Implements,
            TokenKind::Abstract => Node_::Abstract,
            TokenKind::Final => Node_::Final,
            TokenKind::Static => Node_::Static,
            _ => Node_::Ignored,
        })
    }

    fn make_missing(&mut self, _: usize) -> Self::R {
        Ok(Node_::Ignored)
    }

    fn make_list(&mut self, items: Vec<Self::R>, _: usize) -> Self::R {
        let result = if !items.is_empty()
            && !items.iter().all(|r| match r {
                Ok(Node_::Ignored) => true,
                _ => false,
            }) {
            let items = try_collect(items)?;
            Node_::List(items)
        } else {
            Node_::Ignored
        };
        Ok(result)
    }

    fn make_qualified_name(&mut self, arg0: Self::R) -> Self::R {
        Ok(match arg0? {
            Node_::Ignored => Node_::Ignored,
            Node_::List(nodes) => Node_::QualifiedName(nodes),
            node => Node_::QualifiedName(vec![node]),
        })
    }

    fn make_simple_type_specifier(&mut self, arg0: Self::R) -> Self::R {
        arg0
    }

    fn make_simple_initializer(&mut self, _arg0: Self::R, arg1: Self::R) -> Self::R {
        arg1
    }

    fn make_literal_expression(&mut self, arg0: Self::R) -> Self::R {
        arg0
    }

    fn make_list_item(&mut self, item: Self::R, sep: Self::R) -> Self::R {
        Ok(match (item?, sep?) {
            (Node_::Ignored, Node_::Ignored) => Node_::Ignored,
            (x, Node_::Ignored) | (Node_::Ignored, x) => x,
            (x, y) => Node_::ListItem(Box::new((x, y))),
        })
    }

    fn make_generic_type_specifier(
        &mut self,
        class_type: Self::R,
        _argument_list: Self::R,
    ) -> Self::R {
        class_type
    }

    fn make_enum_declaration(
        &mut self,
        attributes: Self::R,
        _keyword: Self::R,
        name: Self::R,
        _colon: Self::R,
        _base: Self::R,
        _type: Self::R,
        _left_brace: Self::R,
        _enumerators: Self::R,
        _right_brace: Self::R,
    ) -> Self::R {
        let (name, attributes) = (name?, attributes?);
        Ok(match name {
            Node_::Ignored => Node_::Ignored,
            _ => Node_::EnumDecl(Box::new(EnumDeclChildren { name, attributes })),
        })
    }

    fn make_alias_declaration(
        &mut self,
        attributes: Self::R,
        _keyword: Self::R,
        name: Self::R,
        _generic_params: Self::R,
        _constraint: Self::R,
        _equal: Self::R,
        _type: Self::R,
        _semicolon: Self::R,
    ) -> Self::R {
        let (name, attributes) = (name?, attributes?);
        Ok(match name {
            Node_::Ignored => Node_::Ignored,
            _ => Node_::TypeAliasDecl(Box::new(TypeAliasDeclChildren { name, attributes })),
        })
    }

    fn make_define_expression(
        &mut self,
        _keyword: Self::R,
        _left_paren: Self::R,
        args: Self::R,
        _right_paren: Self::R,
    ) -> Self::R {
        match args? {
            Node_::List(mut nodes) => {
                if let Some(_snd) = nodes.pop() {
                    if let Some(fst @ Node_::String(_)) = nodes.pop() {
                        if nodes.is_empty() {
                            return Ok(Node_::Define(Box::new(fst)));
                        }
                    }
                }
            }
            _ => (),
        };
        Ok(Node_::Ignored)
    }

    fn make_function_declaration(
        &mut self,
        _attributes: Self::R,
        header: Self::R,
        body: Self::R,
    ) -> Self::R {
        Ok(match (header?, body?) {
            (Node_::Ignored, Node_::Ignored) => Node_::Ignored,
            (v, Node_::Ignored) | (Node_::Ignored, v) => v,
            (v1, v2) => Node_::List(vec![v1, v2]),
        })
    }

    fn make_function_declaration_header(
        &mut self,
        _modifiers: Self::R,
        _keyword: Self::R,
        name: Self::R,
        _type_params: Self::R,
        _left_parens: Self::R,
        _param_list: Self::R,
        _right_parens: Self::R,
        _colon: Self::R,
        _type: Self::R,
        _where: Self::R,
    ) -> Self::R {
        Ok(match name? {
            Node_::Ignored => Node_::Ignored,
            name => Node_::FunctionDecl(Box::new(name)),
        })
    }

    fn make_trait_use(
        &mut self,
        _keyword: Self::R,
        names: Self::R,
        _semicolon: Self::R,
    ) -> Self::R {
        Ok(match names? {
            Node_::Ignored => Node_::Ignored,
            names => Node_::TraitUseClause(Box::new(names)),
        })
    }

    fn make_require_clause(
        &mut self,
        _keyword: Self::R,
        kind: Self::R,
        name: Self::R,
        _semicolon: Self::R,
    ) -> Self::R {
        Ok(match name? {
            Node_::Ignored => Node_::Ignored,
            name => match kind? {
                Node_::Extends => Node_::RequireExtendsClause(Box::new(name)),
                Node_::Implements => Node_::RequireImplementsClause(Box::new(name)),
                _ => Node_::Ignored,
            },
        })
    }

    fn make_const_declaration(
        &mut self,
        _arg0: Self::R,
        _arg1: Self::R,
        hint: Self::R,
        decls: Self::R,
        _arg4: Self::R,
    ) -> Self::R {
        // None of the Node_::Ignoreds should happen in a well-formed file, but they could happen in
        // a malformed one.
        let hint = hint?;
        Ok(match decls? {
            Node_::List(nodes) => match nodes.as_slice() {
                [Node_::List(nodes)] => match nodes.as_slice() {
                    [name, _] => {
                        let name = get_name("", name)?;
                        match hint {
                            Node_::Hint(hv, pos) => {
                                let reason = Reason::Rhint(pos.clone());
                                let ty_ = match hv {
                                    HintValue::String => Ty_::Tprim(aast::Tprim::Tstring),
                                    HintValue::Int => Ty_::Tprim(aast::Tprim::Tint),
                                    HintValue::Float => Ty_::Tprim(aast::Tprim::Tfloat),
                                    HintValue::Num => Ty_::Tprim(aast::Tprim::Tnum),
                                    HintValue::Bool => Ty_::Tprim(aast::Tprim::Tbool),
                                    HintValue::Apply(gn) => Ty_::Tapply(
                                        Id(pos, "\\".to_string() + &(gn.to_unescaped_string())),
                                        Vec::new(),
                                    ),
                                };
                                self.state
                                    .decls
                                    .consts
                                    .insert(name, Ty(reason, Box::new(ty_)));
                            }
                            n => {
                                return Err(format!(
                                    "Expected primitive value for constant {}, but was {:?}",
                                    name, n
                                ))
                            }
                        };
                        Node_::Ignored
                    }
                    _ => Node_::Ignored,
                },
                _ => Node_::Ignored,
            },
            _ => Node_::Ignored,
        })
    }

    fn make_constant_declarator(&mut self, name: Self::R, initializer: Self::R) -> Self::R {
        let (name, initializer) = (name?, initializer?);
        Ok(match name {
            Node_::Ignored => Node_::Ignored,
            _ => Node_::List(vec![name, initializer]),
        })
    }

    fn make_namespace_declaration(
        &mut self,
        _keyword: Self::R,
        name: Self::R,
        body: Self::R,
    ) -> Self::R {
        let (name, body) = (name?, body?);
        Ok(match body {
            Node_::Ignored => Node_::Ignored,
            _ => Node_::NamespaceDecl(Box::new(name), Box::new(body)),
        })
    }

    fn make_namespace_body(
        &mut self,
        _left_brace: Self::R,
        decls: Self::R,
        _right_brace: Self::R,
    ) -> Self::R {
        decls
    }

    fn make_namespace_empty_body(&mut self, _semicolon: Self::R) -> Self::R {
        Ok(Node_::EmptyBody)
    }

    fn make_methodish_declaration(
        &mut self,
        _attributes: Self::R,
        _function_decl_header: Self::R,
        body: Self::R,
        _semicolon: Self::R,
    ) -> Self::R {
        let body = body?;
        Ok(match body {
            Node_::Ignored => Node_::Ignored,
            _ => Node_::MethodDecl(Box::new(body)),
        })
    }

    fn make_classish_declaration(
        &mut self,
        attributes: Self::R,
        modifiers: Self::R,
        keyword: Self::R,
        name: Self::R,
        _type_params: Self::R,
        _extends_keyword: Self::R,
        extends: Self::R,
        _implements_keyword: Self::R,
        implements: Self::R,
        constrs: Self::R,
        body: Self::R,
    ) -> Self::R {
        let name = name?;
        Ok(match name {
            Node_::Ignored => Node_::Ignored,
            _ => Node_::ClassDecl(Box::new(ClassDeclChildren {
                modifiers: modifiers?,
                kind: keyword?,
                name,
                attributes: attributes?,
                extends: extends?,
                implements: implements?,
                constrs: constrs?,
                body: body?,
            })),
        })
    }

    fn make_classish_body(
        &mut self,
        _left_brace: Self::R,
        elements: Self::R,
        _right_brace: Self::R,
    ) -> Self::R {
        elements
    }

    fn make_old_attribute_specification(
        &mut self,
        _left_double_angle: Self::R,
        attributes: Self::R,
        _right_double_angle: Self::R,
    ) -> Self::R {
        attributes
    }

    fn make_attribute_specification(&mut self, attributes: Self::R) -> Self::R {
        attributes
    }

    fn make_attribute(&mut self, _at: Self::R, attibute: Self::R) -> Self::R {
        attibute
    }

    fn make_constructor_call(
        &mut self,
        class_type: Self::R,
        _left_paren: Self::R,
        argument_list: Self::R,
        _right_paren: Self::R,
    ) -> Self::R {
        Ok(Node_::ListItem(Box::new((class_type?, argument_list?))))
    }

    fn make_decorated_expression(&mut self, _decorator: Self::R, expression: Self::R) -> Self::R {
        expression
    }

    fn make_scope_resolution_expression(
        &mut self,
        qualifier: Self::R,
        _operator: Self::R,
        name: Self::R,
    ) -> Self::R {
        Ok(Node_::ScopeResolutionExpression(Box::new((
            qualifier?, name?,
        ))))
    }
}
