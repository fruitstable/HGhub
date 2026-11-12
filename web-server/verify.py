"""Verify a running deployment without printing credentials or upstream URLs.
Usage: python3 web-server/verify.py ORIGIN PASSWORD_FILE REPORT_FILE
Run on loopback first; repeat against the HTTPS public origin after deployment.
"""
import sys,json,urllib.request,urllib.parse,urllib.error,http.cookiejar,pathlib,datetime
origin,password_file,report_file=sys.argv[1:]
origin=origin.rstrip('/'); report={'origin':origin,'checked_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'checks':[]}
class NoRedirect(urllib.request.HTTPRedirectHandler):
 def redirect_request(self,*args,**kwargs):return None
anon=urllib.request.build_opener(NoRedirect())
client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
def fetch(path,data=None,headers=None,opener=client):
 req=urllib.request.Request(origin+path,data=data,headers=headers or {})
 try:
  with opener.open(req,timeout=120) as r:return r.status,r.headers,r.read()
 except urllib.error.HTTPError as e:return e.code,e.headers,e.read()
def check(name,ok,**details):
 report['checks'].append({'name':name,'passed':bool(ok),**details});print(name+': '+('PASS' if ok else 'FAIL'),flush=True)
 if not ok:raise RuntimeError(name+' failed')
def rpc(cmd,args):
 status,_,body=fetch('/api/invoke/'+cmd,json.dumps(args).encode(),{'Content-Type':'application/json','X-HG-Request':'1'})
 check(cmd,status==200,status=status)
 return json.loads(body)
def proxy(route,url):return '/media/'+route+'?u='+urllib.parse.quote(url,safe='')
try:
 s,h,b=fetch('/',opener=anon);check('anonymous_redirect',s==303 and h.get('Location')=='/login',status=s)
 s,_,_=fetch('/media/hls?u=x',opener=anon);check('anonymous_media_denied',s==401,status=s)
 pw=pathlib.Path(password_file).read_text().strip()
 s,_,b=fetch('/login',urllib.parse.urlencode({'password':pw}).encode(),{'Content-Type':'application/x-www-form-urlencoded','Origin':origin});check('password_login',s==200 and b'<div id="root">' in b,status=s)
 feed=rpc('api_feed',{'page':1,'size':8});items=feed.get('items',[]);check('real_feed',len(items)>0,item_count=len(items))
 item=items[0];detail=rpc('api_video',{'id':item['id']});check('detail',detail.get('id')==item['id'] and bool(detail.get('title')))
 episodes=detail.get('episodes') or [{'ep_num':1}];play=rpc('api_play',{'id':item['id'],'ep':episodes[0]['ep_num']});check('play_url',bool(play.get('video_url')))
 s,_,b=fetch(proxy('hls',play['video_url']));check('hls_playlist',s==200 and b.startswith(b'#EXTM3U'),status=s,bytes=len(b))
 text=b.decode();links=[x.strip() for x in text.splitlines() if x.strip() and not x.startswith('#')]
 for _ in range(4):
  check('playlist_media_routes',bool(links) and all(x.startswith('/media/') for x in links))
  s,h,b=fetch(links[0]);check('media_response',s==200,status=s,bytes=len(b))
  if not b.startswith(b'#EXTM3U'):break
  text=b.decode();links=[x.strip() for x in text.splitlines() if x.strip() and not x.startswith('#')]
 check('video_segment',s==200 and len(b)>0 and not b.startswith(b'#EXTM3U'),status=s,bytes=len(b))
 s,h,b=fetch(proxy('img',detail.get('cover') or item['cover']));check('cover',s==200 and h.get('Content-Type','').startswith('image/') and len(b)>0,status=s,bytes=len(b))
 report['passed']=True
except Exception as e:
 report['passed']=False;report['error_type']=type(e).__name__
finally:
 pathlib.Path(report_file).write_text(json.dumps(report,ensure_ascii=False,indent=2));print('Report saved: '+report_file)
sys.exit(0 if report.get('passed') else 1)
