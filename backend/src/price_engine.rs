use std::alloc::System;
// TODO: implement
use std::collections::{HashMap, VecDeque};
use chrono::{Utc, Duration};
use crate::models::PriceTick;

pub struct Pricestore{
    price_store : HashMap<String, VecDeque<PriceTick>>
}

impl Pricestore{
    pub fn new() -> Self{
        Self{
            price_store : HashMap::new()
        }
    }

    pub fn push_tick(&mut self, tick:PriceTick){
        let deque = self
            .price_store
            .entry(tick.symbol.clone())
            .or_insert_with(VecDeque::new);
        
        deque.push_back(tick);
        let cut = Utc::now() - Duration::hours(24);

        while let Some(front) = deque.front() {
            if front.timestamp < cut{
                deque.pop_front();
            }else{
                break;
            } 
                
        }
    }
}





