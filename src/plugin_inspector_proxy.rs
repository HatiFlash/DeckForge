//! A per-inspector, loopback-only HTTP proxy. WebView2 is forced through it rather than
//! relying solely on navigation events (which can fire after a speculative TCP connect).
use std::{io::{Read,Write}, net::{Shutdown,TcpListener,TcpStream}, sync::{Arc,atomic::{AtomicUsize,Ordering}}, time::Duration};

const MAX_CONNECTIONS:usize=16;

pub(crate) struct LocalProxy {pub port:u16, _worker:std::thread::JoinHandle<()>}

impl LocalProxy {
 pub fn start(initial:&str,ws_port:u16)->std::io::Result<Self>{
  let http_port=url::Url::parse(initial).ok().and_then(|u|u.port()).ok_or_else(||std::io::Error::other("invalid PI origin"))?;
  let listener=TcpListener::bind("127.0.0.1:0")?;
  let port=listener.local_addr()?.port();let count=Arc::new(AtomicUsize::new(0));
  let worker=std::thread::spawn(move||for client in listener.incoming(){
   let Ok(client)=client else {break};
   if count.fetch_add(1,Ordering::AcqRel)>=MAX_CONNECTIONS {count.fetch_sub(1,Ordering::AcqRel);continue;}
   let active=count.clone();std::thread::spawn(move||{
    struct Release(Arc<AtomicUsize>);
    impl Drop for Release {fn drop(&mut self){self.0.fetch_sub(1,Ordering::AcqRel);}}
    let _guard=Release(active);let _=forward(client,http_port,ws_port);
   });
  });
  Ok(Self{port,_worker:worker})
 }
}

fn destination(method:&str,target:&str,http_port:u16,ws_port:u16)->Option<(u16,String,bool)>{
 if method=="CONNECT" {
  // HTTPS tunnel host only; the exact EasyPI paths are enforced by the WebView2 resource filter.
  if target=="cdn.jsdelivr.net:443" {return Some((0,"cdn.jsdelivr.net:443".into(),true))}
  if target==format!("127.0.0.1:{http_port}") || target==format!("localhost:{http_port}"){return Some((http_port,String::new(),true))}
  if target==format!("127.0.0.1:{ws_port}") || target==format!("localhost:{ws_port}"){return Some((ws_port,String::new(),true))}
  return None;
 }
 if method!="GET" {return None;}
 if target.starts_with('/') {
  return Some((http_port, target.to_owned(), false));
 }
 let u=url::Url::parse(target).ok()?;
 let port=if u.scheme()=="http" && u.port()==Some(http_port){http_port}
     else if u.scheme()=="ws" && u.port()==Some(ws_port){ws_port}else{return None};
 if u.host_str()!=Some("127.0.0.1") || !u.username().is_empty() || u.password().is_some()
    || u.fragment().is_some() || u.as_str()!=target {return None}
 let mut path=u.path().to_owned();if let Some(q)=u.query(){path.push('?');path.push_str(q);}
 Some((port,path,false))
}

fn forward(mut client:TcpStream,http_port:u16,ws_port:u16)->std::io::Result<()> {
 client.set_read_timeout(Some(Duration::from_secs(3)))?;
 let mut header=Vec::with_capacity(2048);let mut buf=[0u8;1024];
 let end=loop {if header.len()>8192{return Ok(())}
  let n=client.read(&mut buf)?;if n==0{return Ok(())}header.extend_from_slice(&buf[..n]);
  if let Some(pos)=header.windows(4).position(|part|part==b"\r\n\r\n"){break pos+4}
 };
 let request=std::str::from_utf8(&header[..end]).map_err(std::io::Error::other)?;
 let first=request.split("\r\n").next().unwrap_or("");let mut fields=first.split(' ');
 let (Some(method),Some(target),Some(version),None)=(fields.next(),fields.next(),fields.next(),fields.next()) else{return Ok(())};
 if version!="HTTP/1.1" {return Ok(())}
 let Some((port,path,connect))=destination(method,target,http_port,ws_port) else {
  client.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;return Ok(())
 };
 let mut origin=if port==0 {TcpStream::connect(path.as_str())?} else {TcpStream::connect(("127.0.0.1",port))?};
 if connect {
  client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
  if end<header.len(){origin.write_all(&header[end..])?;}
 } else {
  origin.write_all(format!("{method} {path} {version}\r\n").as_bytes())?;
  origin.write_all(&header[first.len()+2..])?;
 }
 client.set_read_timeout(None)?;
 let mut from_client=client.try_clone()?;let mut to_origin=origin.try_clone()?;
 let upload=std::thread::spawn(move||{
  let _=std::io::copy(&mut from_client,&mut to_origin);
  let _=to_origin.shutdown(Shutdown::Write);
 });
 let _=std::io::copy(&mut origin,&mut client);
 let _=client.shutdown(Shutdown::Both);let _=upload.join();Ok(())
}

#[cfg(test)] mod tests {use super::*;
 #[test] fn restricts_proxy_destinations(){
  assert!(destination("GET","http://127.0.0.1:4321/ok?pi=x",4321,5678).is_some());
  assert!(destination("GET","ws://127.0.0.1:5678/",4321,5678).is_some());
  assert!(destination("CONNECT","127.0.0.1:5678",4321,5678).is_some());
  for target in ["http://evil.test/","http://127.0.0.1:4322/leak","http://localhost:4321/", "http://127.0.0.1:4321@evil.test/","http://127.0.0.1:4321/#fragment"] {
   assert!(destination("GET",target,4321,5678).is_none(),"{target}");
  }
  assert!(destination("CONNECT","evil.test:443",4321,5678).is_none());
  assert!(destination("CONNECT","cdn.jsdelivr.net:443",4321,5678).is_some());
  assert!(destination("CONNECT","cdn.jsdelivr.net:80",4321,5678).is_none());
 }
}
