# Phase 2: 사용자 공간 (Userspace) 설계 문서

## 개요

Phase 2는 KPIO 운영체제에서 실질적인 사용자 경험을 제공하는 단계입니다. IPC 시스템, 파일시스템, 네트워킹, 그리고 WASM 기반 쉘을 구현합니다.

---

## 선행 조건

- Phase 1 완료 (WASM 실행, 기본 WASI, VirtIO-Blk)

## 완료 조건

- WASM 쉘에서 파일 시스템 탐색 가능
- TCP 연결 수립
- 프로세스 간 메시지 전달 동작

---

## 1. IPC 시스템

### 1.1 Capability 기반 IPC

```rust
// kernel/src/ipc/capability.rs

use bitflags::bitflags;
use core::sync::atomic::{AtomicU64, Ordering};

/// Capability ID 생성기
static NEXT_CAP_ID: AtomicU64 = AtomicU64::new(1);

/// Capability ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityId(pub u64);

impl CapabilityId {
    pub fn new() -> Self {
        CapabilityId(NEXT_CAP_ID.fetch_add(1, Ordering::SeqCst))
    }
}

bitflags! {
    /// Capability 권한
    pub struct CapabilityRights: u32 {
        /// 읽기
        const READ = 1 << 0;
        /// 쓰기
        const WRITE = 1 << 1;
        /// 실행
        const EXECUTE = 1 << 2;
        /// 메시지 전송
        const SEND = 1 << 3;
        /// 메시지 수신
        const RECEIVE = 1 << 4;
        /// 복제
        const DUPLICATE = 1 << 5;
        /// 전달 (다른 프로세스에 위임)
        const TRANSFER = 1 << 6;
        /// 권한 축소
        const DIMINISH = 1 << 7;
    }
}

/// Capability
#[derive(Debug, Clone)]
pub struct Capability {
    /// 고유 ID
    pub id: CapabilityId,
    /// 대상 리소스
    pub resource: ResourceId,
    /// 권한
    pub rights: CapabilityRights,
    /// 소유자 태스크
    pub owner: TaskId,
    /// 만료 시간 (옵션)
    pub expiry: Option<u64>,
}

impl Capability {
    /// 권한 확인
    pub fn has_rights(&self, required: CapabilityRights) -> bool {
        self.rights.contains(required)
    }
    
    /// 권한 축소된 사본 생성
    pub fn diminish(&self, new_rights: CapabilityRights) -> Option<Capability> {
        if !self.has_rights(CapabilityRights::DIMINISH) {
            return None;
        }
        
        // 새 권한은 기존 권한의 부분집합이어야 함
        if !self.rights.contains(new_rights) {
            return None;
        }
        
        Some(Capability {
            id: CapabilityId::new(),
            resource: self.resource,
            rights: new_rights,
            owner: self.owner,
            expiry: self.expiry,
        })
    }
}

/// 리소스 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceId {
    /// 포트
    Port(PortId),
    /// 메모리 영역
    Memory(MemoryRegionId),
    /// 파일
    File(FileId),
    /// 디바이스
    Device(DeviceId),
}
```

### 1.2 메시지 전달

```rust
// kernel/src/ipc/message.rs

use alloc::vec::Vec;
use super::capability::Capability;

/// 메시지 헤더
#[derive(Debug, Clone)]
pub struct MessageHeader {
    /// 발신자 태스크
    pub sender: TaskId,
    /// 메시지 타입
    pub msg_type: MessageType,
    /// 페이로드 길이
    pub payload_len: u32,
    /// 전달할 Capability 수
    pub cap_count: u8,
}

/// 메시지 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// 일반 데이터
    Data,
    /// 요청 (응답 필요)
    Request,
    /// 응답
    Reply,
    /// 알림 (응답 불필요)
    Notification,
}

/// IPC 메시지
#[derive(Debug)]
pub struct Message {
    pub header: MessageHeader,
    /// 페이로드 데이터
    pub payload: Vec<u8>,
    /// 전달할 Capability
    pub capabilities: Vec<Capability>,
}

/// 최대 메시지 크기
pub const MAX_MESSAGE_SIZE: usize = 4096;
/// 최대 Capability 전달 수
pub const MAX_CAPS_PER_MESSAGE: usize = 8;
```

### 1.3 IPC 포트

```rust
// kernel/src/ipc/port.rs

use alloc::collections::VecDeque;
use spin::Mutex;
use super::message::Message;
use super::capability::{Capability, CapabilityRights};

/// 포트 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(pub u64);

/// IPC 포트
pub struct Port {
    /// 포트 ID
    pub id: PortId,
    /// 메시지 큐
    queue: Mutex<VecDeque<Message>>,
    /// 최대 큐 크기
    max_queue_size: usize,
    /// 대기 중인 수신자
    waiting_receivers: Mutex<VecDeque<TaskId>>,
    /// 대기 중인 발신자
    waiting_senders: Mutex<VecDeque<(TaskId, Message)>>,
}

impl Port {
    pub fn new(id: PortId) -> Self {
        Port {
            id,
            queue: Mutex::new(VecDeque::new()),
            max_queue_size: 64,
            waiting_receivers: Mutex::new(VecDeque::new()),
            waiting_senders: Mutex::new(VecDeque::new()),
        }
    }
    
    /// 메시지 전송 (비동기)
    pub fn send(&self, msg: Message, cap: &Capability) -> Result<(), IpcError> {
        if !cap.has_rights(CapabilityRights::SEND) {
            return Err(IpcError::PermissionDenied);
        }
        
        let mut queue = self.queue.lock();
        
        if queue.len() >= self.max_queue_size {
            return Err(IpcError::QueueFull);
        }
        
        queue.push_back(msg);
        
        // 대기 중인 수신자 깨우기
        let mut receivers = self.waiting_receivers.lock();
        if let Some(task_id) = receivers.pop_front() {
            crate::task::SCHEDULER.lock().unblock(task_id);
        }
        
        Ok(())
    }
    
    /// 메시지 수신 (블로킹)
    pub fn receive(&self, cap: &Capability) -> Result<Message, IpcError> {
        if !cap.has_rights(CapabilityRights::RECEIVE) {
            return Err(IpcError::PermissionDenied);
        }
        
        loop {
            let mut queue = self.queue.lock();
            
            if let Some(msg) = queue.pop_front() {
                return Ok(msg);
            }
            
            // 큐가 비어있으면 대기
            drop(queue);
            
            let current = crate::task::SCHEDULER.lock().current()
                .ok_or(IpcError::NoCurrentTask)?;
            
            self.waiting_receivers.lock().push_back(current);
            crate::task::SCHEDULER.lock().block_current();
        }
    }
    
    /// 메시지 수신 시도 (논블로킹)
    pub fn try_receive(&self, cap: &Capability) -> Result<Option<Message>, IpcError> {
        if !cap.has_rights(CapabilityRights::RECEIVE) {
            return Err(IpcError::PermissionDenied);
        }
        
        let mut queue = self.queue.lock();
        Ok(queue.pop_front())
    }
}

#[derive(Debug)]
pub enum IpcError {
    PermissionDenied,
    QueueFull,
    PortNotFound,
    InvalidCapability,
    NoCurrentTask,
}
```

### 1.4 시스템 서비스 포트

```rust
// kernel/src/ipc/services.rs

use lazy_static::lazy_static;
use alloc::collections::BTreeMap;
use spin::Mutex;

/// 시스템 서비스 레지스트리
lazy_static! {
    pub static ref SERVICE_REGISTRY: Mutex<ServiceRegistry> = 
        Mutex::new(ServiceRegistry::new());
}

/// 서비스 레지스트리
pub struct ServiceRegistry {
    services: BTreeMap<ServiceName, PortId>,
}

/// 시스템 서비스 이름
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceName {
    /// VFS 서비스
    Vfs,
    /// 네트워크 서비스
    Network,
    /// 디스플레이 서비스 (Phase 3)
    Display,
    /// 입력 서비스 (Phase 3)
    Input,
    /// 사용자 정의 서비스
    Custom(String),
}

impl ServiceRegistry {
    pub fn new() -> Self {
        ServiceRegistry {
            services: BTreeMap::new(),
        }
    }
    
    /// 서비스 등록
    pub fn register(&mut self, name: ServiceName, port: PortId) {
        self.services.insert(name, port);
    }
    
    /// 서비스 조회
    pub fn lookup(&self, name: &ServiceName) -> Option<PortId> {
        self.services.get(name).copied()
    }
}
```

---

## 2. VFS (Virtual File System)

### 2.1 VFS 레이어

```rust
// kernel/src/fs/vfs.rs

use alloc::string::String;
use alloc::vec::Vec;

/// 파일 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Regular,
    Directory,
    SymLink,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
}

/// 파일 메타데이터
#[derive(Debug, Clone)]
pub struct Metadata {
    pub file_type: FileType,
    pub size: u64,
    pub permissions: u16,
    pub uid: u32,
    pub gid: u32,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub inode: u64,
    pub nlink: u32,
}

/// 디렉토리 엔트리
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub inode: u64,
    pub file_type: FileType,
}

/// 파일시스템 Trait
pub trait Filesystem: Send + Sync {
    /// 파일시스템 이름
    fn name(&self) -> &str;
    
    /// 루트 inode
    fn root_inode(&self) -> u64;
    
    /// 파일 조회
    fn lookup(&self, parent: u64, name: &str) -> Result<u64, FsError>;
    
    /// 메타데이터 조회
    fn getattr(&self, inode: u64) -> Result<Metadata, FsError>;
    
    /// 디렉토리 읽기
    fn readdir(&self, inode: u64) -> Result<Vec<DirEntry>, FsError>;
    
    /// 파일 읽기
    fn read(&self, inode: u64, offset: u64, buf: &mut [u8]) -> Result<usize, FsError>;
    
    /// 파일 쓰기
    fn write(&self, inode: u64, offset: u64, buf: &[u8]) -> Result<usize, FsError>;
    
    /// 파일 생성
    fn create(&self, parent: u64, name: &str, mode: u16) -> Result<u64, FsError>;
    
    /// 디렉토리 생성
    fn mkdir(&self, parent: u64, name: &str, mode: u16) -> Result<u64, FsError>;
    
    /// 파일 삭제
    fn unlink(&self, parent: u64, name: &str) -> Result<(), FsError>;
    
    /// 디렉토리 삭제
    fn rmdir(&self, parent: u64, name: &str) -> Result<(), FsError>;
    
    /// 파일 이름 변경
    fn rename(&self, old_parent: u64, old_name: &str, 
              new_parent: u64, new_name: &str) -> Result<(), FsError>;
    
    /// 동기화
    fn sync(&self) -> Result<(), FsError>;
}

#[derive(Debug)]
pub enum FsError {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    NotDirectory,
    IsDirectory,
    NotEmpty,
    NoSpace,
    IoError,
    InvalidArgument,
}
```

### 2.2 마운트 테이블

```rust
// kernel/src/fs/mount.rs

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use spin::RwLock;

/// 마운트 정보
pub struct MountInfo {
    /// 마운트 포인트
    pub mount_point: String,
    /// 파일시스템
    pub filesystem: Arc<dyn Filesystem>,
    /// 디바이스 (옵션)
    pub device: Option<String>,
    /// 읽기 전용
    pub readonly: bool,
}

/// 마운트 테이블
pub struct MountTable {
    mounts: RwLock<BTreeMap<String, MountInfo>>,
}

impl MountTable {
    pub fn new() -> Self {
        MountTable {
            mounts: RwLock::new(BTreeMap::new()),
        }
    }
    
    /// 마운트
    pub fn mount(
        &self,
        mount_point: String,
        fs: Arc<dyn Filesystem>,
        device: Option<String>,
        readonly: bool,
    ) -> Result<(), FsError> {
        let mut mounts = self.mounts.write();
        
        if mounts.contains_key(&mount_point) {
            return Err(FsError::AlreadyExists);
        }
        
        mounts.insert(mount_point.clone(), MountInfo {
            mount_point,
            filesystem: fs,
            device,
            readonly,
        });
        
        Ok(())
    }
    
    /// 언마운트
    pub fn unmount(&self, mount_point: &str) -> Result<(), FsError> {
        let mut mounts = self.mounts.write();
        
        mounts.remove(mount_point)
            .ok_or(FsError::NotFound)?;
        
        Ok(())
    }
    
    /// 경로에서 파일시스템 찾기
    pub fn resolve(&self, path: &str) -> Option<(Arc<dyn Filesystem>, String)> {
        let mounts = self.mounts.read();
        
        // 가장 긴 매칭 마운트 포인트 찾기
        let mut best_match: Option<(&str, &MountInfo)> = None;
        
        for (mount_point, info) in mounts.iter() {
            if path.starts_with(mount_point.as_str()) {
                match best_match {
                    None => best_match = Some((mount_point, info)),
                    Some((best_mp, _)) if mount_point.len() > best_mp.len() => {
                        best_match = Some((mount_point, info));
                    }
                    _ => {}
                }
            }
        }
        
        best_match.map(|(mp, info)| {
            let relative = path.strip_prefix(mp).unwrap_or("/");
            (info.filesystem.clone(), relative.to_string())
        })
    }
}
```

### 2.3 ext4 파일시스템

```rust
// kernel/src/fs/ext4/mod.rs

use super::vfs::{Filesystem, Metadata, DirEntry, FileType, FsError};
use crate::driver::virtio::blk::VirtioBlkDevice;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

/// ext4 슈퍼블록
#[repr(C)]
pub struct Ext4Superblock {
    pub s_inodes_count: u32,
    pub s_blocks_count_lo: u32,
    pub s_r_blocks_count_lo: u32,
    pub s_free_blocks_count_lo: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_cluster_size: u32,
    pub s_blocks_per_group: u32,
    pub s_clusters_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,  // 0xEF53
    // ... (전체 필드는 storage/src/fs/ext4.rs 참조)
}

/// ext4 파일시스템
pub struct Ext4Filesystem {
    device: Arc<Mutex<VirtioBlkDevice>>,
    superblock: Ext4Superblock,
    block_size: u32,
}

impl Ext4Filesystem {
    /// 블록 디바이스에서 ext4 마운트
    pub fn mount(device: Arc<Mutex<VirtioBlkDevice>>) -> Result<Self, FsError> {
        let mut sb_buf = [0u8; 1024];
        
        // 슈퍼블록 읽기 (오프셋 1024)
        {
            let mut dev = device.lock();
            dev.read_block(2, &mut sb_buf) // sector 2, 3
                .map_err(|_| FsError::IoError)?;
        }
        
        // 슈퍼블록 파싱
        let superblock: Ext4Superblock = unsafe {
            core::ptr::read(sb_buf.as_ptr() as *const _)
        };
        
        // 매직 넘버 확인
        if superblock.s_magic != 0xEF53 {
            return Err(FsError::InvalidArgument);
        }
        
        let block_size = 1024 << superblock.s_log_block_size;
        
        Ok(Ext4Filesystem {
            device,
            superblock,
            block_size,
        })
    }
    
    /// inode 읽기
    fn read_inode(&self, inode_num: u64) -> Result<Ext4Inode, FsError> {
        let group = ((inode_num - 1) / self.superblock.s_inodes_per_group as u64) as u32;
        let index = ((inode_num - 1) % self.superblock.s_inodes_per_group as u64) as u32;
        
        // 그룹 디스크립터에서 inode 테이블 위치 조회
        // inode 읽기
        // ...
        todo!()
    }
}

impl Filesystem for Ext4Filesystem {
    fn name(&self) -> &str {
        "ext4"
    }
    
    fn root_inode(&self) -> u64 {
        2 // ext4 루트 inode는 항상 2
    }
    
    fn lookup(&self, parent: u64, name: &str) -> Result<u64, FsError> {
        let parent_inode = self.read_inode(parent)?;
        // 디렉토리 엔트리 검색
        todo!()
    }
    
    fn getattr(&self, inode: u64) -> Result<Metadata, FsError> {
        let inode_data = self.read_inode(inode)?;
        // Metadata 변환
        todo!()
    }
    
    fn readdir(&self, inode: u64) -> Result<Vec<DirEntry>, FsError> {
        todo!()
    }
    
    fn read(&self, inode: u64, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        todo!()
    }
    
    fn write(&self, inode: u64, offset: u64, buf: &[u8]) -> Result<usize, FsError> {
        todo!()
    }
    
    fn create(&self, parent: u64, name: &str, mode: u16) -> Result<u64, FsError> {
        todo!()
    }
    
    fn mkdir(&self, parent: u64, name: &str, mode: u16) -> Result<u64, FsError> {
        todo!()
    }
    
    fn unlink(&self, parent: u64, name: &str) -> Result<(), FsError> {
        todo!()
    }
    
    fn rmdir(&self, parent: u64, name: &str) -> Result<(), FsError> {
        todo!()
    }
    
    fn rename(&self, old_parent: u64, old_name: &str,
              new_parent: u64, new_name: &str) -> Result<(), FsError> {
        todo!()
    }
    
    fn sync(&self) -> Result<(), FsError> {
        // 캐시 플러시
        todo!()
    }
}
```

---

## 3. TCP/IP 네트워킹 (smoltcp)

### 3.1 네트워크 인터페이스

```rust
// kernel/src/net/interface.rs

//! 네트워크 인터페이스 관리
//!
//! smoltcp 의존성 필수 피처:
//! - `medium-ethernet`: Ethernet 프레임 지원
//! - `proto-ipv4`: IPv4 프로토콜
//! - `socket-tcp`: TCP 소켓
//! - `socket-udp`: UDP 소켓
//! - `socket-dhcpv4`: DHCP 클라이언트
//! - `socket-dns`: DNS 리졸버

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::{Device, Medium};
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address};
use alloc::vec::Vec;

/// 네트워크 인터페이스 관리자
pub struct NetworkManager {
    interfaces: Vec<NetworkInterface>,
}

pub struct NetworkInterface {
    /// 인터페이스 이름
    pub name: String,
    /// smoltcp 인터페이스
    iface: Interface,
    /// 디바이스
    device: VirtioNetDevice,
    /// 소켓 세트
    sockets: SocketSet<'static>,
}

impl NetworkManager {
    pub fn new() -> Self {
        NetworkManager {
            interfaces: Vec::new(),
        }
    }
    
    /// VirtIO-Net 디바이스 추가
    pub fn add_virtio_device(&mut self, device: VirtioNetDevice) {
        let mac = device.mac_address();
        let ethernet_addr = EthernetAddress(mac);
        
        let config = Config::new(ethernet_addr.into());
        
        let mut iface = Interface::new(config, &mut device, smoltcp::time::Instant::ZERO);
        
        // IP 주소 설정 (DHCP 전까지 정적)
        iface.update_ip_addrs(|addrs| {
            addrs.push(IpCidr::new(Ipv4Address::new(10, 0, 2, 15).into(), 24)).unwrap();
        });
        
        // 게이트웨이 설정
        iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).unwrap();
        
        let sockets = SocketSet::new(Vec::new());
        
        self.interfaces.push(NetworkInterface {
            name: format!("eth{}", self.interfaces.len()),
            iface,
            device,
            sockets,
        });
    }
    
    /// 네트워크 폴링 (메인 루프에서 호출)
    pub fn poll(&mut self, timestamp: smoltcp::time::Instant) {
        for interface in &mut self.interfaces {
            interface.iface.poll(timestamp, &mut interface.device, &mut interface.sockets);
        }
    }
}
```

### 3.2 TCP 소켓 API

```rust
// kernel/src/net/tcp.rs

use smoltcp::socket::tcp::{Socket, SocketBuffer, State};
use smoltcp::wire::{IpEndpoint, IpAddress, Ipv4Address};
use alloc::vec::Vec;

/// TCP 소켓 핸들
pub struct TcpSocket {
    handle: smoltcp::iface::SocketHandle,
}

impl TcpSocket {
    /// 새 TCP 소켓 생성
    pub fn new(rx_buffer_size: usize, tx_buffer_size: usize) -> Self {
        let rx_buffer = SocketBuffer::new(vec![0u8; rx_buffer_size]);
        let tx_buffer = SocketBuffer::new(vec![0u8; tx_buffer_size]);
        
        let socket = Socket::new(rx_buffer, tx_buffer);
        
        // 전역 소켓셋에 추가
        let handle = NETWORK_MANAGER.lock()
            .interfaces[0]
            .sockets
            .add(socket);
        
        TcpSocket { handle }
    }
    
    /// 연결
    pub fn connect(&mut self, addr: Ipv4Address, port: u16) -> Result<(), NetError> {
        let endpoint = IpEndpoint::new(IpAddress::Ipv4(addr), port);
        
        let mut manager = NETWORK_MANAGER.lock();
        let iface = &mut manager.interfaces[0];
        
        let socket = iface.sockets.get_mut::<Socket>(self.handle);
        
        socket.connect(iface.iface.context(), endpoint, 49152)
            .map_err(|_| NetError::ConnectionFailed)?;
        
        Ok(())
    }
    
    /// 리스닝
    pub fn listen(&mut self, port: u16) -> Result<(), NetError> {
        let mut manager = NETWORK_MANAGER.lock();
        let socket = manager.interfaces[0].sockets
            .get_mut::<Socket>(self.handle);
        
        socket.listen(port)
            .map_err(|_| NetError::BindFailed)?;
        
        Ok(())
    }
    
    /// 데이터 송신
    pub fn send(&mut self, data: &[u8]) -> Result<usize, NetError> {
        let mut manager = NETWORK_MANAGER.lock();
        let socket = manager.interfaces[0].sockets
            .get_mut::<Socket>(self.handle);
        
        socket.send_slice(data)
            .map_err(|_| NetError::SendFailed)
    }
    
    /// 데이터 수신
    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        let mut manager = NETWORK_MANAGER.lock();
        let socket = manager.interfaces[0].sockets
            .get_mut::<Socket>(self.handle);
        
        socket.recv_slice(buf)
            .map_err(|_| NetError::RecvFailed)
    }
    
    /// 연결 상태 확인
    pub fn state(&self) -> TcpState {
        let manager = NETWORK_MANAGER.lock();
        let socket = manager.interfaces[0].sockets
            .get::<Socket>(self.handle);
        
        match socket.state() {
            State::Closed => TcpState::Closed,
            State::Listen => TcpState::Listen,
            State::SynSent => TcpState::Connecting,
            State::Established => TcpState::Connected,
            State::CloseWait | State::LastAck => TcpState::Closing,
            _ => TcpState::Other,
        }
    }
    
    /// 연결 종료
    pub fn close(&mut self) {
        let mut manager = NETWORK_MANAGER.lock();
        let socket = manager.interfaces[0].sockets
            .get_mut::<Socket>(self.handle);
        
        socket.close();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    Connecting,
    Connected,
    Closing,
    Other,
}

#[derive(Debug)]
pub enum NetError {
    ConnectionFailed,
    BindFailed,
    SendFailed,
    RecvFailed,
    NotConnected,
}
```

### 3.3 WASI 네트워크 확장

```rust
// runtime/src/wasi/net.rs

use wasmtime::*;
use super::WasiCtx;

/// WASI sock_open 구현 (WASI Preview 2 스타일)
pub fn sock_open(
    mut caller: Caller<'_, WasiCtx>,
    address_family: i32,
    socket_type: i32,
    fd_ptr: i32,
) -> i32 {
    let socket = match (address_family, socket_type) {
        (0, 0) => {
            // AF_INET, SOCK_STREAM -> TCP
            TcpSocket::new(65536, 65536)
        }
        _ => return wasi::ERRNO_INVAL as i32,
    };
    
    // FD 할당
    let fd = caller.data_mut().allocate_socket_fd(socket);
    
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .unwrap();
    
    let data = memory.data_mut(&mut caller);
    data[fd_ptr as usize..fd_ptr as usize + 4]
        .copy_from_slice(&fd.to_le_bytes());
    
    wasi::ERRNO_SUCCESS as i32
}

/// WASI sock_connect 구현
pub fn sock_connect(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,
    addr_ptr: i32,
    addr_len: i32,
) -> i32 {
    // 주소 파싱 및 연결
    // ...
    todo!()
}
```

---

## 4. WASM 쉘

### 4.1 쉘 WASM 모듈

```rust
// wasm-apps/shell/src/lib.rs (WASM으로 컴파일)

use std::io::{self, Write, BufRead};

fn main() {
    println!("KPIO Shell v0.1");
    println!("Type 'help' for available commands.\n");
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    loop {
        print!("kpio> ");
        stdout.flush().unwrap();
        
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap() == 0 {
            break;
        }
        
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts[0];
        let args = &parts[1..];
        
        match cmd {
            "help" => cmd_help(),
            "ls" => cmd_ls(args),
            "cd" => cmd_cd(args),
            "pwd" => cmd_pwd(),
            "cat" => cmd_cat(args),
            "mkdir" => cmd_mkdir(args),
            "rm" => cmd_rm(args),
            "echo" => cmd_echo(args),
            "clear" => cmd_clear(),
            "exit" => break,
            _ => println!("Unknown command: {}", cmd),
        }
    }
    
    println!("Goodbye!");
}

fn cmd_help() {
    println!("Available commands:");
    println!("  help        - Show this help");
    println!("  ls [path]   - List directory contents");
    println!("  cd <path>   - Change directory");
    println!("  pwd         - Print working directory");
    println!("  cat <file>  - Display file contents");
    println!("  mkdir <dir> - Create directory");
    println!("  rm <path>   - Remove file or directory");
    println!("  echo <text> - Print text");
    println!("  clear       - Clear screen");
    println!("  exit        - Exit shell");
}

fn cmd_ls(args: &[&str]) {
    let path = args.get(0).unwrap_or(&".");
    
    match std::fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let file_type = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        "d"
                    } else {
                        "-"
                    };
                    println!("{} {}", file_type, entry.file_name().to_string_lossy());
                }
            }
        }
        Err(e) => println!("ls: {}: {}", path, e),
    }
}

fn cmd_cd(args: &[&str]) {
    let path = args.get(0).unwrap_or(&"/");
    if let Err(e) = std::env::set_current_dir(path) {
        println!("cd: {}: {}", path, e);
    }
}

fn cmd_pwd() {
    match std::env::current_dir() {
        Ok(path) => println!("{}", path.display()),
        Err(e) => println!("pwd: {}", e),
    }
}

fn cmd_cat(args: &[&str]) {
    for path in args {
        match std::fs::read_to_string(path) {
            Ok(contents) => print!("{}", contents),
            Err(e) => println!("cat: {}: {}", path, e),
        }
    }
}

fn cmd_mkdir(args: &[&str]) {
    for path in args {
        if let Err(e) = std::fs::create_dir(path) {
            println!("mkdir: {}: {}", path, e);
        }
    }
}

fn cmd_rm(args: &[&str]) {
    for path in args {
        let result = if std::fs::metadata(path)
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            std::fs::remove_dir(path)
        } else {
            std::fs::remove_file(path)
        };
        
        if let Err(e) = result {
            println!("rm: {}: {}", path, e);
        }
    }
}

fn cmd_echo(args: &[&str]) {
    println!("{}", args.join(" "));
}

fn cmd_clear() {
    // ANSI escape sequence
    print!("\x1B[2J\x1B[H");
}
```

### 4.2 완전한 WASI 파일시스템 구현

```rust
// runtime/src/wasi/fs.rs

use super::WasiCtx;
use wasmtime::*;

/// fd_prestat_get - 사전 열린 디렉토리 정보
pub fn fd_prestat_get(
    caller: Caller<'_, WasiCtx>,
    fd: i32,
    buf_ptr: i32,
) -> i32 {
    // 사전 열린 디렉토리 정보 반환
    // ...
    todo!()
}

/// path_open - 경로로 파일 열기
pub fn path_open(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,           // 기준 디렉토리 FD
    dirflags: i32,     // lookup 플래그
    path_ptr: i32,     // 경로 포인터
    path_len: i32,     // 경로 길이
    oflags: i32,       // open 플래그
    fs_rights_base: i64,
    fs_rights_inheriting: i64,
    fdflags: i32,
    opened_fd_ptr: i32,
) -> i32 {
    // 경로 파싱
    let memory = caller.get_export("memory")
        .and_then(|e| e.into_memory())
        .unwrap();
    
    let data = memory.data(&caller);
    let path_bytes = &data[path_ptr as usize..(path_ptr + path_len) as usize];
    let path = core::str::from_utf8(path_bytes).unwrap();
    
    // VFS를 통해 파일 열기
    // ...
    todo!()
}

/// fd_read - 파일 읽기
pub fn fd_read(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,
    iovs_ptr: i32,
    iovs_len: i32,
    nread_ptr: i32,
) -> i32 {
    // iovec 파싱 및 파일 읽기
    // ...
    todo!()
}

/// fd_readdir - 디렉토리 읽기
pub fn fd_readdir(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,
    buf_ptr: i32,
    buf_len: i32,
    cookie: i64,
    bufused_ptr: i32,
) -> i32 {
    // 디렉토리 엔트리 열거
    // ...
    todo!()
}

/// fd_seek - 파일 오프셋 변경
pub fn fd_seek(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,
    offset: i64,
    whence: i32,
    newoffset_ptr: i32,
) -> i32 {
    // 오프셋 변경
    // ...
    todo!()
}

/// fd_close - 파일 닫기
pub fn fd_close(
    mut caller: Caller<'_, WasiCtx>,
    fd: i32,
) -> i32 {
    caller.data_mut().close_fd(fd as u32);
    wasi::ERRNO_SUCCESS as i32
}
```

---

## 5. 병렬 작업

Phase 2 진행 중 병렬로 수행 가능한 작업:

| 작업 | 의존성 | 비고 |
|------|--------|------|
| FAT32 파일시스템 | VFS 인터페이스 완료 | USB 드라이브 지원 |
| DHCP 클라이언트 | 네트워크 스택 완료 | 자동 IP 설정 |
| DNS 리졸버 | TCP 구현 완료 | 도메인 이름 해석 |
| LVM 지원 | VFS 완료 | 4.3 제안 구현 |

---

## 6. 검증 체크리스트

Phase 2 완료 전 확인 사항:

- [ ] IPC 포트 생성 및 메시지 전송
- [ ] Capability 권한 검증 동작
- [ ] VFS 마운트/언마운트
- [ ] ext4 파일 읽기/쓰기
- [ ] 디렉토리 목록 조회
- [ ] TCP 연결 수립
- [ ] HTTP GET 요청 성공
- [ ] WASM 쉘 ls/cd/cat 동작
- [ ] 파일 생성/삭제
- [ ] 서비스 레지스트리 동작
