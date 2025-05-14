/**
* filename : linked_list
* author : HAMA
* date: 2025. 5. 11.
* description: 
**/

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::collections::HashMap;

type Link<T> = Option<Rc<RefCell<Node<T>>>>;

#[derive(Debug)]
pub struct Node<T> {
  pub value: T,
  prev: Option<Weak<RefCell<Node<T>>>>,
  next: Link<T>,
}

#[derive(Debug)]
pub struct DoublyLinkedList<T> {
  head: Link<T>,
  tail: Link<T>,
  count: usize,
}

impl<T> DoublyLinkedList<T> {
  pub fn new() -> Self {
    DoublyLinkedList { head: None, tail: None , count: 0}
  }
  
  pub fn push_back(&mut self, value: T) -> Rc<RefCell<Node<T>>> {
    let new_node = Rc::new(RefCell::new(Node {value, prev: None, next: None }));
    
    match self.tail.take() {
      Some(old_tail) => {
        old_tail.borrow_mut().next = Some(new_node.clone());
        new_node.borrow_mut().prev = Some(Rc::downgrade(&old_tail));
        self.tail = Some(new_node.clone());
      }
      None => {
        self.head = Some(new_node.clone());
        self.tail = Some(new_node.clone());
      }
    }
    self.count += 1;
    new_node
  }
  
  pub fn remove(&mut self, node: Rc<RefCell<Node<T>>>) {
    let prev = node.borrow().prev.clone();
    let next = node.borrow().next.clone();
    
    if let Some(ref prev_weak) = prev {
      if let Some(prev) = prev_weak.upgrade() {
        prev.borrow_mut().next = next.clone();
      }
    } else {
      // Removing head
      self.head = next.clone();
    }
    
    if let Some(next) = next {
      next.borrow_mut().prev = prev;
    } else {
      self.tail = prev.and_then(|w| w.upgrade());
    }
    
    self.count -= 1;
  }
  
  pub fn pop_front(&mut self) -> Option<Rc<RefCell<Node<T>>>> {
    self.head.take().map(|old_head| {
      if let Some(next) = old_head.borrow_mut().next.take() {
        next.borrow_mut().prev.take();
        self.head = Some(next);
      } else {
        self.tail.take();
      }
      self.count = self.count.saturating_sub(1);
      old_head
    })
  }
  
  pub fn peek_front(&self) -> Option<Rc<RefCell<Node<T>>>> {
    self.head.as_ref().map(|node| node.clone())
  }
  
  pub fn is_empty(&self) -> bool {
    self.head.is_none()
  }
  
  /// 리스트 내 노드 개수 반환
  pub fn len(&self) -> usize {
    self.count
  }
}
