// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

extern crate core_markdown as p;
use std::io::{Read,Write,BufWriter};
fn safe<T:std::fmt::Debug>(f:impl FnOnce()->T)->String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(x)=>format!("{x:?}"), Err(_)=>"PANIC".into()
    }
}
fn main() {
    std::panic::set_hook(Box::new(|_|{}));
    let mut bytes=vec![];std::io::stdin().read_to_end(&mut bytes).unwrap();
    let mut output=BufWriter::new(std::io::stdout().lock());
    let (mut at,mut n)=(0,0);
    while at<bytes.len() {
        let len=u32::from_le_bytes(bytes[at..at+4].try_into().unwrap())as usize;at+=4;
        let source=std::str::from_utf8(&bytes[at..at+len]).unwrap();at+=len;
        for mode in 0..3 {
            let mut options=if mode==0 {p::Options::default()} else {p::Options::gfm()};
            if mode==1 {
                options.parse.constructs.gfm_footnote_definition=false;
                options.parse.constructs.gfm_label_start_footnote=false;
            }
            options.compile.allow_dangerous_html=true;
            options.compile.allow_dangerous_protocol=true;
            let direct=safe(||p::to_html_with_options(source,&options).map_err(|e|e.to_string()));
            let checked=safe(||{
                let mut out=String::new();
                p::EventParser::new(source,&options.parse)
                    .and_then(|events|p::render_events(source,events,&options.compile,&mut out))
                    .map(|_|out).map_err(|e|e.to_string())
            });
            let arena=safe(||p::Document::parse(source,&options.parse).and_then(|document|{
                let mut out=String::new();
                p::render_events(source,document.events(),&options.compile,&mut out).map(|_|out)
            }).map_err(|e|e.to_string()));
            let events=safe(||p::EventParser::new(source,&options.parse).map(|events|{
                events.map(|event|event.map_err(|e|e.to_string())).collect::<Vec<_>>()
            }).map_err(|e|e.to_string()));
            let ast=safe(||p::Document::parse(source,&options.parse).map(|document|{
                format!("{:?}",document.events().collect::<Vec<_>>())
            }).map_err(|e|e.to_string()));
            writeln!(output,"{n}\t{mode}\t{direct}\t{checked}\t{arena}\t{events}\t{ast}").unwrap();
        }
        n+=1;
    }
}
