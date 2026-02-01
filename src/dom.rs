use blitz::{
    dom::{
        ns, Attribute as BlitzAttribute, BaseDocument, DocGuard, DocGuardMut,
        Document as BlitzDocument, DocumentConfig, EventDriver, EventHandler,
        FontContext, LocalName, Namespace, QualName, BULLET_FONT, DEFAULT_CSS,
    },
    html::{DocumentHtmlParser, HtmlProvider},
    traits::events::{DomEvent, DomEventKind, EventState, UiEvent},
};
use napi::{bindgen_prelude::*, Env};
use napi_derive::napi;
use parley::fontique::Blob;
use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    rc::Rc,
    str::FromStr,
    sync::Arc,
};

#[napi]
pub struct Document {
    pub(crate) env: Env,
    pub(crate) doc: Rc<RefCell<DocumentHolder>>,
    pub(crate) nodes: HashMap<usize, Reference<Node>>,
}

#[napi(object)]
pub struct DocumentInitConfig {
    pub ua_stylesheets: Option<Vec<String>>,
    pub base_html: Option<String>,
}

pub(crate) struct DocumentInitConfigFinal {
    pub ua_stylesheets: Vec<String>,
    pub base_html: String,
}

impl From<Option<DocumentInitConfig>> for DocumentInitConfigFinal {
    fn from(value: Option<DocumentInitConfig>) -> Self {
        let value = value.unwrap_or(DocumentInitConfig {
            ua_stylesheets: None,
            base_html: None,
        });
        DocumentInitConfigFinal {
            ua_stylesheets: value.ua_stylesheets.unwrap_or(vec![DEFAULT_CSS.to_string()]),
            base_html: value.base_html.unwrap_or(String::from(BASE_HTML)),
        }
    }
}

pub struct DocumentHolder {
    pub(crate) base: BaseDocument,
    pub(crate) event_handler: SimpleEventHandler,
}

impl Deref for DocumentHolder {
    type Target = BaseDocument;
    fn deref(&self) -> &BaseDocument {
        &self.base
    }
}

impl DerefMut for DocumentHolder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Default, Clone)]
pub struct SimpleEventHandler {
    pub(crate) listeners: HashMap<ListenerKey, Vec<Rc<(Env, FunctionRef<(), ()>)>>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ListenerKey(usize, u8);

impl EventHandler for SimpleEventHandler {
    fn handle_event(
        &mut self,
        _chain: &[usize],
        event: &mut DomEvent,
        _doc: &mut dyn BlitzDocument,
        _event_state: &mut EventState,
    ) {
        let Some(handles) = self
            .listeners
            .get(&ListenerKey(event.target, event.data.kind().discriminant()))
        else {
            return;
        };

        println!(
            "Matched ========== node_id: {} event {:?}",
            event.target, event
        );

        for pair in handles {
            let (env, handler) = pair.as_ref();
            let Err(e) = handler
                .borrow_back(env)
                .map(|handler| handler.call(()))
                .flatten()
            else {
                return;
            };
            eprintln!("{}", e.reason)
        }
    }
}

impl BlitzDocument for DocumentHolder {
    fn inner(&self) -> DocGuard<'_> {
        self.base.inner()
    }

    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        self.base.inner_mut()
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        if !matches!(event, UiEvent::PointerMove(_)) {
            println!("handle ui event: {:?}", event);
        }
        EventDriver::new(&mut self.base, self.event_handler.clone()).handle_ui_event(event);
    }

    /*fn poll(&mut self, task_context: Option<TaskContext>) -> bool {
        todo!()
    }*/

    /*fn id(&self) -> usize {
        todo!()
    }*/
}

#[napi]
pub struct Node {
    pub(crate) id: usize,
    pub(crate) doc: Rc<RefCell<DocumentHolder>>,
    pub(crate) props: HashMap<String, ObjectRef>,
}

#[napi(object)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

static BASE_HTML: &str = r#"<!DOCTYPE html>
<html>
    <head></head>
    <body id="body"></body>
</html>
"#;

impl Document {
    pub(crate) fn construct(env: Env, init_config: DocumentInitConfigFinal) -> Document {
        let mut font_ctx = FontContext::new();
        font_ctx
            .collection
            .register_fonts(Blob::new(Arc::new(BULLET_FONT) as _), None);

        let config = DocumentConfig {
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            ua_stylesheets: Some(init_config.ua_stylesheets),
            ..DocumentConfig::default()
        };

        let holder = DocumentHolder {
            base: BaseDocument::new(config),
            event_handler: SimpleEventHandler::default(),
        };

        Document {
            env,
            doc: Rc::new(RefCell::new(holder)),
            nodes: Default::default(),
        }
    }
}

#[napi]
impl Document {
    #[napi(constructor)]
    pub fn new(env: Env, init_config: Option<DocumentInitConfig>) -> Document {
        Self::construct(env, init_config.into())
    }
}

#[napi]
impl Document {
    #[napi]
    pub fn load_html(&mut self, html: String) {
        let base = &mut self.doc.borrow_mut().base;
        let mut mutr = base.mutate();
        DocumentHtmlParser::parse_into_mutator(&mut mutr, html.as_str());
        drop(mutr);
        base.resolve(0.0);
    }

    #[napi]
    pub fn resolve(&mut self, current_time_for_animations: f64) {
        self.doc.borrow_mut().resolve(current_time_for_animations);
    }

    fn node_reference(&mut self, node_id: usize) -> Result<Reference<Node>> {
        let node = Node {
            id: node_id,
            doc: Rc::clone(&self.doc),
            props: Default::default(),
        };

        let node_reference = node.into_reference(self.env)?;
        self.nodes.insert(node_id, node_reference.clone(self.env)?);

        Ok(node_reference)
    }

    #[napi]
    pub fn get_node(&mut self, id: f64) -> Result<Option<Reference<Node>>> {
        let id = id as usize;
        if let Some(node_reference) = self.nodes.get(&id) {
            return match Reference::clone(node_reference, self.env) {
                Err(err) => Err(err),
                Ok(node_reference) => Ok(Some(node_reference)),
            };
        }

        let Some(id) = self.doc.borrow().get_node(id) else {
            return Ok(None);
        };

        self.node_reference(id)
            .map(|node_reference| Some(node_reference))
    }

    #[napi]
    pub fn deep_clone_node(&mut self, node: Reference<Node>) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().deep_clone_node(node.id);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_element2(
        &mut self,
        name: String,
        namespace: Option<String>,
        attrs: HashMap<String, String>,
    ) -> Result<Reference<Node>> {
        let attrs = attrs
            .into_iter()
            .map(|(name, value)| Attribute { name, value })
            .collect::<Vec<_>>();
        let id = self.doc.borrow_mut().create_element(name, namespace, attrs);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_element(
        &mut self,
        name: String,
        namespace: Option<String>,
        attrs: Vec<Attribute>,
    ) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().create_element(name, namespace, attrs);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_text_node(&mut self, text: String) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().create_text_node(text);
        self.node_reference(id)
    }

    #[napi]
    pub fn create_comment_node(&mut self) -> Result<Reference<Node>> {
        let id = self.doc.borrow_mut().create_comment_node();
        self.node_reference(id)
    }

    #[napi]
    pub fn create_comment_node_with_content(&mut self, content: String) -> Result<Reference<Node>> {
        let id = self
            .doc
            .borrow_mut()
            .create_comment_node_with_content(content);
        self.node_reference(id)
    }

    #[napi]
    pub fn insert(
        &mut self,
        node: Option<Reference<Node>>,
        parent: Option<Reference<Node>>,
        anchor: Option<Reference<Node>>,
    ) {
        self.doc.borrow_mut().insert(
            node.map(|node| node.id),
            parent.map(|node| node.id),
            anchor.map(|node| node.id),
        );
    }

    #[napi]
    pub fn next_sibling(&mut self, node: Reference<Node>) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().next_sibling(node.id);
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn previous_sibling(&mut self, node: Reference<Node>) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().previous_sibling(node.id);
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn parent_node(&mut self, node: Reference<Node>) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().parent_node(node.id);
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn patch_prop(&mut self, node: Reference<Node>, name: String, value: String, namespace: Option<String>) {
        self.doc.borrow_mut().patch_prop(node.id, name, value, namespace);
    }

    #[napi]
    pub fn set_style_property(&mut self, node: Reference<Node>, name: String, value: String) {
        self.doc
            .borrow_mut()
            .set_style_property(node.id, name, value);
    }

    #[napi]
    pub fn remove_style_property(&mut self, node: Reference<Node>, name: String) {
        self.doc
            .borrow_mut()
            .remove_style_property(node.id, name);
    }

    #[napi]
    pub fn query_selector(&mut self, selector: String) -> Result<Option<Reference<Node>>> {
        let id = self.doc.borrow_mut().query_selector(selector)?;
        Ok(match id {
            Some(id) => Some(self.node_reference(id)?),
            None => None,
        })
    }

    #[napi]
    pub fn remove(&mut self, node: Reference<Node>) {
        self.doc.borrow_mut().remove(node.id);
    }

    #[napi]
    pub fn set_element_text(&mut self, node: Reference<Node>, text: String) {
        self.doc.borrow_mut().set_element_text(node.id, text);
    }

    #[napi]
    pub fn set_text(&mut self, node: Reference<Node>, text: String) {
        self.doc.borrow_mut().set_text(node.id, text);
    }
}

pub(crate) fn qual_name(local_name: &str, namespace: Option<&str>) -> QualName {
    QualName {
        prefix: None,
        ns: namespace.map(Namespace::from).unwrap_or(ns!(html)),
        local: LocalName::from(local_name),
    }
}

impl DocumentHolder {
    pub fn resolve(&mut self, current_time_for_animations: f64) {
        self.base.resolve(current_time_for_animations);
    }

    pub fn get_node(&self, id: usize) -> Option<usize> {
        let node = self.base.get_node(id).unwrap();

        Some(node.id)
    }

    pub fn create_element(&mut self, name: String, namespace: Option<String>, attrs: Vec<Attribute>) -> usize {
        self.base.mutate().create_element(
            qual_name(&name, namespace.as_deref()),
            attrs
                .iter()
                .map(|attr| BlitzAttribute {
                    name: qual_name(&attr.name, namespace.as_deref()),
                    value: attr.value.clone(),
                })
                .collect(),
        )
    }

    pub fn deep_clone_node(&mut self, node_id: usize) -> usize {
        self.base.mutate().deep_clone_node(node_id)
    }

    pub fn create_text_node(&mut self, text: String) -> usize {
        self.base.mutate().create_text_node(&text)
    }

    pub fn create_comment_node(&mut self) -> usize {
        self.base.mutate().create_comment_node()
    }

    pub fn create_comment_node_with_content(&mut self, content: String) -> usize {
        let id = self.create_comment_node();

        self.set_element_text(id, content);

        id
    }

    pub fn insert(
        &mut self,
        node_id: Option<usize>,
        parent_id: Option<usize>,
        anchor_id: Option<usize>,
    ) {
        match (node_id, parent_id, anchor_id) {
            (None, _, _) | (_, None, None) => return,
            (Some(node), _, Some(anchor)) => {
                self.base.mutate().insert_nodes_after(anchor, &[node]);
            }
            (Some(node), Some(parent), None) => {
                self.base.mutate().append_children(parent, &[node]);
            }
        }
    }

    pub fn next_sibling(&mut self, node_id: usize) -> Option<usize> {
        self.base
            .get_node(node_id)
            .and_then(|node| node.forward(1).map(|node| node.id))
    }

    pub fn previous_sibling(&mut self, node_id: usize) -> Option<usize> {
        self.base
            .get_node(node_id)
            .and_then(|node| node.backward(1).map(|node| node.id))
    }

    pub fn parent_node(&mut self, node_id: usize) -> Option<usize> {
        self.base.mutate().parent_id(node_id)
    }

    pub fn patch_prop(&mut self, node_id: usize, name: String, value: String, namespace: Option<String>) {
        self.base.mutate().set_attribute(
            node_id,
            qual_name(&name, namespace.as_deref()),
            &value,
        );
    }

    pub fn set_style_property(&mut self, node_id: usize, name: String, value: String) {
        self.base
            .mutate()
            .set_style_property(node_id, &name, &value);
        println!("set style property {} = {}", name, value);
    }

    pub fn remove_style_property(&mut self, node_id: usize, name: String) {
        self.base
            .mutate()
            .remove_style_property(node_id, &name);
        println!("remove style property {}", name);
    }

    pub fn query_selector(&mut self, selector: String) -> Result<Option<usize>> {
        self.base
            .query_selector(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector error: {:?}", err)))
    }

    pub fn remove(&mut self, node_id: usize) {
        self.base.mutate().remove_node(node_id);
    }

    pub fn set_element_text(&mut self, node_id: usize, text: String) {
        let Some(node) = self.base.get_node_mut(node_id) else {
            return;
        };

        let children = node.children.iter().map(|child| *child).collect::<Vec<_>>();

        let mut found = false;
        for child in children {
            if !self.base.get_node_mut(child).unwrap().is_text_node() {
                continue;
            }
            if found {
                self.base.mutate().remove_node(child);
                continue;
            }
            found = true;
            self.base.mutate().set_node_text(child, &text);
        }

        if !found {
            let text_node_id = self.create_text_node(text);
            self.insert(Some(text_node_id), Some(node_id), None);
        }
    }

    pub fn set_text(&mut self, node_id: usize, text: String) {
        if let Some(data) = self
            .base
            .get_node_mut(node_id)
            .and_then(|node| node.text_data_mut())
        {
            data.content = text
        }
    }

    pub fn add_event_listener(
        &mut self,
        node: usize,
        kind: DomEventKind,
        env: Env,
        event_handler: FunctionRef<(), ()>,
    ) {
        let pair = &Rc::new((env, event_handler));
        self.event_handler
            .listeners
            .entry(ListenerKey(node, kind.discriminant()))
            .and_modify(|pairs| pairs.append(&mut vec![Rc::clone(pair)]))
            .or_insert(vec![Rc::clone(pair)]);
    }

    pub fn remove_event_listener(
        &mut self,
        node: usize,
        kind: DomEventKind,
        _event_handler: FunctionRef<(), ()>,
    ) {
        self.event_handler
            .listeners
            .remove(&ListenerKey(node, kind.discriminant()));
        // todo: remove by same function
    }
}

#[napi]
impl Node {
    #[napi]
    pub fn print_tree(&mut self, level: Option<f64>) {
        let Some(level) = level else { return };
        self.doc
            .borrow()
            .base
            .get_node(self.id)
            .unwrap()
            .print_tree(level.round() as usize);
    }

    #[napi]
    pub fn self_prop(&mut self, name: String, value: ObjectRef) {
        self.props.insert(name, value);
    }

    #[napi]
    pub fn add_event_listener(
        &mut self,
        env: Env,
        event_type: String,
        handler: FunctionRef<(), ()>,
    ) {
        let Ok(event_kind) = DomEventKind::from_str(&event_type) else {
            return eprintln!("unsupported event type {}", event_type);
        };

        self.doc
            .borrow_mut()
            .add_event_listener(self.id, event_kind, env, handler);
    }

    #[napi]
    pub fn remove_event_listener(&mut self, event_type: String, handler: FunctionRef<(), ()>) {
        let Ok(event_kind) = DomEventKind::from_str(&event_type) else {
            return eprintln!("unsupported event type {}", event_type);
        };

        self.doc
            .borrow_mut()
            .remove_event_listener(self.id, event_kind, handler);
    }
}
