#!/usr/bin/env python3
"""Pinned static recovery for the audited a.txt and b.txt; no Lua or VM execution."""
from __future__ import annotations
import argparse,contextlib,io,json,re,sys
from pathlib import Path
import ab_primitives as u
import ab_analyze,ab_payload,ab_micro,ab_lift
PIN={
 'a':{'size':163115,'sha256':'4db86d4772a6a4d7515863ccb52949fbdf4270643f6bcb2b24fb8aab1edf0883','view_sha256':'1989f670edb55c540c34e6a044b4f527f589b56af6c34252ae1194f0cdbbc0b6'},
 'b':{'size':182491,'sha256':'b2b41d2180acfc380f3c9fef81e07ccb8c2e3734442c4b8851de87049daea659','view_sha256':'38a9680278f3a9ca4b9c63073617110c517e87551df04d6061127e3639da183c'}}

def top_fields(source):
 marks=list(re.finditer(r'^\t([A-Za-z_]\w*|\[\d+\]) = ',source,re.M))
 return {z[1]:source[z.end():marks[i+1].start() if i+1<len(marks) else source.rfind('\n}')].rstrip().removesuffix(',') for i,z in enumerate(marks)}

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('inputs',type=Path,nargs='+');parser.add_argument('-o','--output',type=Path,required=True);args=parser.parse_args()
 if sys.version_info<(3,10):parser.error('Python 3.10+ is required')
 if sys.flags.optimize:parser.error('Do not use -O: this audited reproducer requires assertions enabled')
 if args.output.exists():parser.error('Output directory already exists; refusing to overwrite')
 selected={};views={};viewdir=Path(__file__).resolve().parent/'profile_views'
 for path in args.inputs:
  data=path.read_bytes();name=next((k for k,v in PIN.items() if len(data)==v['size'] and u.sha256(data)==v['sha256']),None)
  if name is None:parser.error('Input does not match either audited sample: '+str(path))
  if name in selected:parser.error('Duplicate input sample: '+name)
  view=(viewdir/(name+'.formatted.luau')).read_bytes()
  if u.sha256(view)!=PIN[name]['view_sha256']:parser.error('Audited source-view SHA mismatch: '+name)
  selected[name]=data;views[name]=view.decode('utf-8')
 args.output.mkdir(parents=True,exist_ok=False);facts={}
 for name in sorted(selected):
  P=args.output/name;P.mkdir();S=views[name];F=top_fields(S);cfg=ab_analyze.CONFIG[name]
  (P/'input.original.txt').write_bytes(selected[name])
  log=io.StringIO()
  with contextlib.redirect_stdout(log):
   encoded=ab_analyze.outer(P,F,cfg);ab_analyze.fold(P,S,encoded,cfg)
   ab_payload.decode(name,P);ab_micro.analyze(name,P);facts[name]=ab_lift.finish(P,name)
  (P/'reproduction.log').write_text(log.getvalue(),encoding='utf-8',newline='\n')
  print('PASS:',name,':',facts[name]['prototype_count'],'prototypes /',facts[name]['record_count'],'records /',facts[name]['fused_record_count'],'fused records')
 sha={str(p.relative_to(args.output)):u.sha256(p.read_bytes()) for p in sorted(args.output.rglob('*')) if p.is_file()}
 result={'status':'PASS','method':'Python data decoding and static source lifting only; no Lua/business execution','scope':'Hash-pinned original inputs plus hash-pinned audited formatted source views; not a general decompiler. Readable templates require a full parsed-core signature.','input_and_view_pins':{k:PIN[k] for k in sorted(selected)},'facts':facts,'generated_file_count':len(sha),'sha256':sha}
 u.save_json(args.output/'verification.json',result)
 print('PASS:',len(sha),'generated-file hashes. Expected outputs below are STATIC derivations, not execution logs:')
 for name in sorted(facts):print(name+':',facts[name]['expected_output'])

if __name__=='__main__':main()
