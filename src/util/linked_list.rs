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

#[cfg(test)]
mod tests {
  use super::*;
  
  #[test]
  fn test_new_list_is_empty() {
    let list: DoublyLinkedList<i32> = DoublyLinkedList::new();
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
    assert!(list.peek_front().is_none());
  }
  
  #[test]
  fn test_push_back() {
    let mut list = DoublyLinkedList::new();
    
    // 첫 번째 노드 추가
    let node1 = list.push_back(10);
    assert_eq!(node1.borrow().value, 10);
    assert!(!list.is_empty());
    assert_eq!(list.len(), 1);
    
    // 두 번째 노드 추가
    let node2 = list.push_back(20);
    assert_eq!(node2.borrow().value, 20);
    assert_eq!(list.len(), 2);
    
    // 세 번째 노드 추가
    let node3 = list.push_back(30);
    assert_eq!(node3.borrow().value, 30);
    assert_eq!(list.len(), 3);
    
    // 링크 확인 (node1 -> node2 -> node3)
    assert!(node1.borrow().prev.is_none());
    assert!(node1.borrow().next.is_some());
    
    let next1 = node1.borrow().next.as_ref().unwrap().clone();
    assert_eq!(next1.borrow().value, 20);
    
    assert!(node2.borrow().prev.is_some());
    assert!(node2.borrow().next.is_some());
    
    let next2 = node2.borrow().next.as_ref().unwrap().clone();
    assert_eq!(next2.borrow().value, 30);
    
    assert!(node3.borrow().prev.is_some());
    assert!(node3.borrow().next.is_none());
  }
  
  #[test]
  fn test_peek_front() {
    let mut list = DoublyLinkedList::new();
    
    // 비어있는 리스트
    assert!(list.peek_front().is_none());
    
    // 노드 추가
    let node1 = list.push_back(10);
    let front = list.peek_front().unwrap();
    
    // 값과 참조 비교
    assert_eq!(front.borrow().value, 10);
    assert!(Rc::ptr_eq(&front, &node1));
    
    // 추가 노드 삽입 후에도 첫 번째 노드 유지
    let node2 = list.push_back(20);
    let front = list.peek_front().unwrap();
    
    assert_eq!(front.borrow().value, 10);
    assert!(Rc::ptr_eq(&front, &node1));
    assert!(!Rc::ptr_eq(&front, &node2));
  }
  
  #[test]
  fn test_pop_front() {
    let mut list = DoublyLinkedList::new();
    
    // 비어있는 리스트에서 pop
    assert!(list.pop_front().is_none());
    
    // 노드 추가
    let node1 = list.push_back(10);
    let node2 = list.push_back(20);
    let node3 = list.push_back(30);
    
    assert_eq!(list.len(), 3);
    
    // 첫 번째 노드 제거
    let popped1 = list.pop_front().unwrap();
    assert_eq!(popped1.borrow().value, 10);
    assert!(Rc::ptr_eq(&popped1, &node1));
    assert_eq!(list.len(), 2);
    
    // 두 번째 노드가 이제 첫 번째가 됨
    let front = list.peek_front().unwrap();
    assert_eq!(front.borrow().value, 20);
    assert!(Rc::ptr_eq(&front, &node2));
    
    // 두 번째 노드 제거
    let popped2 = list.pop_front().unwrap();
    assert_eq!(popped2.borrow().value, 20);
    assert!(Rc::ptr_eq(&popped2, &node2));
    assert_eq!(list.len(), 1);
    
    // 세 번째 노드가 이제 첫 번째가 됨
    let front = list.peek_front().unwrap();
    assert_eq!(front.borrow().value, 30);
    assert!(Rc::ptr_eq(&front, &node3));
    
    // 마지막 노드 제거
    let popped3 = list.pop_front().unwrap();
    assert_eq!(popped3.borrow().value, 30);
    assert!(Rc::ptr_eq(&popped3, &node3));
    assert_eq!(list.len(), 0);
    
    // 리스트가 비어있어야 함
    assert!(list.is_empty());
    assert!(list.peek_front().is_none());
  }
  
  #[test]
  fn test_remove() {
    let mut list = DoublyLinkedList::new();
    
    // 노드 추가
    let node1 = list.push_back(10);
    let node2 = list.push_back(20);
    let node3 = list.push_back(30);
    let node4 = list.push_back(40);
    let node5 = list.push_back(50);
    
    assert_eq!(list.len(), 5);
    
    // 중간 노드 제거 (node3)
    list.remove(node3);
    assert_eq!(list.len(), 4);
    
    // 링크 확인 (node2 -> node4)
    let next2 = node2.borrow().next.as_ref().unwrap().clone();
    assert_eq!(next2.borrow().value, 40);
    assert!(Rc::ptr_eq(&next2, &node4));
    
    let prev4 = node4.borrow().prev.as_ref().unwrap().upgrade().unwrap();
    assert_eq!(prev4.borrow().value, 20);
    assert!(Rc::ptr_eq(&prev4, &node2));
    
    // 첫 번째 노드 제거 (node1)
    list.remove(node1);
    assert_eq!(list.len(), 3);
    
    // 노드2가 이제 첫 번째가 됨
    let front = list.peek_front().unwrap();
    assert_eq!(front.borrow().value, 20);
    assert!(Rc::ptr_eq(&front, &node2));
    assert!(node2.borrow().prev.is_none());
    
    // 마지막 노드 제거 (node5)
    list.remove(node5);
    assert_eq!(list.len(), 2);
    
    // node4가 이제 마지막이 됨
    assert!(node4.borrow().next.is_none());
    
    // 모든 노드 제거 후 비어있는지 확인
    list.remove(node2);
    list.remove(node4);
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());
    assert!(list.peek_front().is_none());
  }
  
  #[test]
  fn test_complex_operations() {
    let mut list = DoublyLinkedList::new();
    
    // 노드 추가
    let node1 = list.push_back(10);
    let node2 = list.push_back(20);
    
    // 중간 노드 제거
    list.remove(node1);
    assert_eq!(list.len(), 1);
    
    // 새 노드 추가
    let node3 = list.push_back(30);
    assert_eq!(list.len(), 2);
    
    // pop_front 실행
    let popped = list.pop_front().unwrap();
    assert_eq!(popped.borrow().value, 20);
    assert!(Rc::ptr_eq(&popped, &node2));
    assert_eq!(list.len(), 1);
    
    // 마지막 남은 노드 확인
    let front = list.peek_front().unwrap();
    assert_eq!(front.borrow().value, 30);
    assert!(Rc::ptr_eq(&front, &node3));
    
    // 마지막 노드 제거
    list.remove(node3);
    assert!(list.is_empty());
  }
  
  #[test]
  fn test_large_list() {
    let mut list = DoublyLinkedList::new();
    let mut nodes = Vec::new();
    
    // 100개 노드 추가
    for i in 0..100 {
      nodes.push(list.push_back(i));
    }
    
    assert_eq!(list.len(), 100);
    
    // 랜덤 위치의 노드 10개 제거
    let indices_to_remove = vec![5, 10, 15, 20, 25, 30, 35, 40, 45, 50];
    for &idx in &indices_to_remove {
      list.remove(nodes[idx].clone());
    }
    
    assert_eq!(list.len(), 90);
    
    // 앞에서 10개 노드 제거
    for _ in 0..10 {
      list.pop_front();
    }
    
    assert_eq!(list.len(), 80);
    
    // 다시 20개 노드 추가
    for i in 100..120 {
      list.push_back(i);
    }
    
    assert_eq!(list.len(), 100);
  }
}