// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{EventParser,Options,render_events};
use core_markdown::stream::EventKind;
use std::{env,fs,fmt};
#[derive(Default)] struct Writes {calls:usize,empty:usize,short:usize,bytes:usize}
impl fmt::Write for Writes {fn write_str(&mut self,s:&str)->fmt::Result{self.calls+=1;self.empty+=usize::from(s.is_empty());self.short+=usize::from(!s.is_empty()&&s.len()<=16);self.bytes+=s.len();Ok(())}}
fn main(){let source=fs::read_to_string(env::args().nth(1).unwrap()).unwrap();let mut options=Options::default();options.compile.allow_dangerous_html=true;options.compile.allow_dangerous_protocol=true;let mut events=0;let mut text=0;let mut short=0;let mut text_bytes=0;let mut code_blocks=0;
for event in EventParser::new(&source,&options.parse).unwrap(){events+=1;match event.unwrap().kind {EventKind::Text(v)|EventKind::SoftBreak(v)=>{let n=v.resolve(&source).unwrap().len();text+=1;short+=usize::from(n<=16);text_bytes+=n},EventKind::CodeBlock{..}=>code_blocks+=1,_=>{}}}
let mut writes=Writes::default();render_events(&source,EventParser::new(&source,&options.parse).unwrap(),&options.compile,&mut writes).unwrap();println!("{{\"source_bytes\":{},\"event_bytes\":{},\"events\":{},\"text_events\":{},\"text_events_at_most_16_bytes\":{},\"text_bytes\":{},\"code_blocks\":{},\"writer_calls\":{},\"empty_writer_calls\":{},\"nonempty_writer_calls_at_most_16_bytes\":{},\"html_bytes\":{}}}",source.len(),std::mem::size_of::<core_markdown::stream::Event>(),events,text,short,text_bytes,code_blocks,writes.calls,writes.empty,writes.short,writes.bytes);}
