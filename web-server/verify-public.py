"""Anonymous site acceptance; never prints media URLs, response bodies or credentials."""
import argparse,datetime,hashlib,json,pathlib,re,sys,urllib.request,urllib.error
p=argparse.ArgumentParser();p.add_argument('origin');p.add_argument('report');p.add_argument('--downloads-manifest');a=p.parse_args();origin=a.origin.rstrip('/')
report={'origin':origin,'checked_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'checks':[]}
def req(path,data=None,headers=None,method=None):
 try:
  with urllib.request.urlopen(urllib.request.Request(origin+path,data=data,headers=headers or {},method=method),timeout=120) as r:return r.status,r.headers,r.read()
 except urllib.error.HTTPError as e:return e.code,e.headers,e.read()
def check(name,ok,**details):
 report['checks'].append({'name':name,'passed':bool(ok),**details});print(name+': '+('PASS' if ok else 'FAIL'),flush=True)
 if not ok:raise RuntimeError(name+' failed')
def rpc(cmd,args,status=200):
 s,h,b=req('/api/invoke/'+cmd,json.dumps(args).encode(),{'Content-Type':'application/json','X-HG-Request':'1'});check(cmd,s==status,status=s);return json.loads(b)
def media(path):
 check('opaque_media_reference',path.startswith('/media/') and '?t=' in path and '?u=' not in path)
 return req(path)
try:
 s,h,b=req('/');check('anonymous_home',s==200 and b'<div id="root">' in b,status=s)
 boot=rpc('app_boot',{});check('public_guest_boot',boot.get('public_access') is True and not boot.get('user'))
 for cmd in ['line_set','security_set','api_login','net_diag']:rpc(cmd,{},403)
 s,_,_=req('/media/img?u=https%3A%2F%2Fexample.com%2Ftest.jpg');check('arbitrary_proxy_rejected',s==403,status=s)
 feed=rpc('api_feed',{'page':1,'size':8});items=feed.get('items',[]);check('real_feed',bool(items),item_count=len(items))
 item=items[0];detail=rpc('api_video',{'id':item['id']});check('detail',detail.get('id')==item['id'] and bool(detail.get('title')))
 ep=(detail.get('episodes') or [{'ep_num':1}])[0]['ep_num'];play=rpc('api_play',{'id':item['id'],'ep':ep})
 s,_,b=media(play['video_url']);check('hls_playlist',s==200 and b.startswith(b'#EXTM3U'),status=s,bytes=len(b))
 for _ in range(4):
  text=b.decode();check('playlist_has_no_raw_targets','?u=' not in text)
  for line in text.splitlines():
   if line.startswith('#EXT-X-KEY:'):
    match=re.search(r'URI="([^"]+)"',line)
    if match:
     ks,_,kb=media(match[1]);check('hls_key',ks==200 and len(kb) in (16,24,32),status=ks,bytes=len(kb))
  links=[x.strip() for x in text.splitlines() if x.strip() and not x.startswith('#')];check('playlist_routes',bool(links) and all(x.startswith('/media/') for x in links))
  s,_,b=media(links[0]);check('media_response',s==200 and bool(b),status=s,bytes=len(b))
  if not b.startswith(b'#EXTM3U'):break
 check('video_segment',s==200 and len(b)>0 and not b.startswith(b'#EXTM3U'),status=s,bytes=len(b))
 s,h,b=media(detail.get('cover') or item['cover']);check('cover',s==200 and h.get('Content-Type','').startswith('image/') and bool(b),status=s,bytes=len(b))
 if a.downloads_manifest:
  expected=json.loads(pathlib.Path(a.downloads_manifest).read_text());s,h,b=req('/downloads/');check('download_page',s==200 and 'Windows' in b.decode() and 'Android' in b.decode() and 'macOS' in b.decode())
  s,h,b=req('/downloads/manifest.json');remote=json.loads(b);check('download_manifest',s==200 and remote==expected)
  for f in expected['files']:
   s,h,b=req(f['url'],method='HEAD');check('download_head',s==200 and int(h['Content-Length'])==f['bytes'] and 'attachment' in h.get('Content-Disposition',''),file=f['name'])
   s,h,b=req(f['url'],headers={'Range':'bytes=0-63'});check('download_range',s==206 and len(b)==64,file=f['name'])
   digest=hashlib.sha256();size=0
   with urllib.request.urlopen(origin+f['url'],timeout=120) as response:
    for chunk in iter(lambda:response.read(262144),b''):digest.update(chunk);size+=len(chunk)
   check('download_sha256',size==f['bytes'] and digest.hexdigest()==f['sha256'],file=f['name'],bytes=size,sha256=digest.hexdigest())
 report['passed']=True
except Exception as e:
 report['passed']=False;report['error_type']=type(e).__name__
finally:
 pathlib.Path(a.report).write_text(json.dumps(report,ensure_ascii=False,indent=2));print('Report saved: '+a.report)
sys.exit(0 if report.get('passed') else 1)
