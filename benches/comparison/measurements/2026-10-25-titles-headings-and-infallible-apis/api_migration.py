"""Adapt staged historical drivers to the infallible core API; archives stay unchanged."""
import re
from pathlib import Path
# Preserve all comments and whitespace; only remove result-handling tokens at
# the two changed call sites, plus Ok wrappers in their equality assertions.
pattern=re.compile(r'//[^\n]*|/\*.*?\*/|(?:br|r)(?P<h>\#*)".*?"(?P=h)|b?"(?:\\.|[^"\\])*"|b?\'(?:\\(?:u\{[^}]*\}|x..|.)|[^\'\\])\'|[A-Za-z_][A-Za-z_0-9]*|[^\s]',re.S)
def migrate(path):
 text=path.read_text(); tokens=[m for m in pattern.finditer(text) if not m[0].startswith(('//','/*'))]; v=[m[0] for m in tokens]
 matching={}; stack=[]
 for i,t in enumerate(v):
  if t in ('(', '[', '{'):stack.append(i)
  elif t in (')',']','}'):
   j=stack.pop(); assert v[j]=={')':'(',']':'[','}':'{'}[t], (path,i)
   matching[j]=i
 edits=[]; calls=[]
 for i,t in enumerate(v):
  if t=='to_html_with_options' and i+1<len(v) and v[i+1]=='(' and (not i or v[i-1]!='fn'):
   if i>=3 and v[i-3:i]==['markdown',':',':']:continue
  elif t=='new' and i>=3 and v[i-3:i]==['EventParser',':',':'] and v[i+1]=='(':pass
  else:continue
  end=matching[i+1]; calls.append(i)
  if v[end+1:end+2]==['?']:edits.append((tokens[end+1].start(),tokens[end+1].end(),''))
  elif v[end+1:end+4] in [['.','unwrap','('],['.','expect','(']]:
   stop=matching[end+3]; edits.append((tokens[end+1].start(),tokens[stop].end(),''))
 for i,t in enumerate(v):
  if t!='assert_eq' or v[i+1:i+3]!=['!','(']:continue
  end=matching[i+2]
  if not any(i<call<end for call in calls):continue
  # Find argument starts without descending into any delimiter pair.
  arg=i+3
  while arg<end:
   stop=arg
   while stop<end and v[stop]!=',':stop=matching.get(stop,stop)+1
   if v[arg:arg+2]==['Ok','('] and matching[arg+1]==stop-1:
    edits.extend([(tokens[arg].start(),tokens[arg+1].end(),''),(tokens[stop-1].start(),tokens[stop-1].end(),'')])
   arg=stop+1
 for start,end,replacement in sorted(set(edits),reverse=True):text=text[:start]+replacement+text[end:]
 if edits:path.write_text(text)
 return len(edits)
