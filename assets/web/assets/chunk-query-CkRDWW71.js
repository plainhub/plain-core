import{a as e,o as t}from"./chunk-client-id-sMazQ4of.js";import"./chunk-core-iC9FAxFL.js";import{y as n}from"./chunk-native-resource-url-IKw68_dV.js";import{$ as r,A as i,F as a,L as o,j as s,lt as c,x as l}from"./chunk-_plugin-vue_export-helper-BqdUuCM-.js";import{T as u,_t as d,a as f,c as p,ct as m,d as h,ot as g,st as _,t as v,yt as y,z as b}from"./chunk-gql-client-C2APrgOK.js";var x=new TextEncoder;function S(e){let t=new Uint8Array(e.reduce((e,t)=>e+4+t.length,0)),n=0;for(let r of e)new DataView(t.buffer).setUint32(n,r.length,!1),t.set(r,n+4),n+=4+r.length;return t}function C(e,t,n){let r=g(24),i=_(e,r,n).encrypt(t),a=new Uint8Array(24+i.length);return a.set(r),a.set(i,24),a}function w(e,t,n){return _(e,t.subarray(0,24),n).decrypt(t.subarray(24))}var T=class{client;key;handshakePath;registration;binding;sent=0n;received=0n;constructor(e,t,n=`/graphql`){this.client=e,this.key=t,this.handshakePath=n;let r=[...g(32)].map(e=>e.toString(16).padStart(2,`0`)).join(``),i=C(t,x.encode(JSON.stringify({timestampMillis:p(),clientNonce:r})),S([x.encode(`plain-graphql-init-v1`),x.encode(n),x.encode(e)]));this.registration=new Uint8Array(1+i.length),this.registration[0]=1,this.registration.set(i,1)}acknowledge(e){if(e[0]!==1||this.binding)throw Error(`Invalid subscription acknowledgement`);let t=S([x.encode(`plain-graphql-ack-v1`),x.encode(this.handshakePath),x.encode(this.client),m(this.registration)]),n=JSON.parse(new TextDecoder().decode(w(this.key,e.subarray(1),t)));if(!/^[0-9a-f]{64}$/.test(n.serverNonce))throw Error(`Invalid subscription server nonce`);this.binding=m(S([this.registration,e]))}aad(e,t){if(!this.binding)throw Error(`Subscription handshake incomplete`);let n=new Uint8Array(8);return new DataView(n.buffer).setBigUint64(0,t,!1),S([x.encode(`plain-graphql-frame-v1`),x.encode(`/graphql`),x.encode(this.client),this.binding,x.encode(e),n])}encode(e){if(this.sent===18446744073709551615n)throw Error(`Subscription sequence exhausted`);let t=C(this.key,x.encode(JSON.stringify(e)),this.aad(`client`,this.sent)),n=new Uint8Array(9+t.length);return n[0]=1,new DataView(n.buffer).setBigUint64(1,this.sent,!1),n.set(t,9),this.sent++,n}decode(e){if(e[0]!==1||e.length<49)throw Error(`Invalid subscription frame`);let t=new DataView(e.buffer,e.byteOffset,e.byteLength).getBigUint64(1,!1);if(t!==this.received||t===18446744073709551615n)throw Error(`Invalid subscription sequence`);let n=w(this.key,e.subarray(9),this.aad(`server`,t));return this.received++,JSON.parse(new TextDecoder().decode(n))}};function E(e,t){return D(e,t)}function D(n,r){let i=new URL(r?d(r,`graphql`):`/graphql`,b()),a=e(),o=y();if(!o)throw Error(`Subscription session missing`);i.searchParams.set(`cid`,a);let s=h(i.toString(),t(),`plain-graphql-ws-v1`);s.binaryType=`arraybuffer`;let c=!0,l=!1,f,p,m,g=setTimeout(()=>v(Error(`Subscription handshake timeout`)),1e4),_={send(e){try{if(!c||!l||!f)throw Error(`Subscription connection unavailable`);s.send(f.encode(e))}catch(e){v(e)}},close(){c=!1,f=void 0,clearTimeout(g),clearTimeout(m),clearInterval(p),s.close()}};function v(e){c&&(_.close(),n.dropped(e))}return s.onopen=()=>{if(c){if(s.protocol!==`plain-graphql-ws-v1`){v(Error(`Subscription protocol mismatch`));return}try{f=new T(a,u(o)),s.send(f.registration)}catch(e){v(e)}}},s.onmessage=e=>{if(c)try{if(!f||!(e.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(!l)f.acknowledge(new Uint8Array(e.data)),l=!0,clearTimeout(g),n.opened(),p=setInterval(()=>{_.send({type:`ping`}),m=setTimeout(()=>v(Error(`Subscription heartbeat timeout`)),1e4)},2e4);else{let t=f.decode(new Uint8Array(e.data));typeof t==`object`&&t&&`type`in t&&t.type===`pong`?clearTimeout(m):n.message(t)}}catch(e){v(e)}},s.onerror=()=>v(Error(`Subscription connection error`)),s.onclose=()=>v(Error(`Subscription connection closed`)),_}var O=new class{openConnection;constructor(e=E){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&this.settled(t.id),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>e.message===`resync_required`)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!n()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},k=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,A=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,j=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,M=`
  fragment AppFragment on App {
    clientId
    urlToken
    httpPort
    httpsPort
    appDir
    deviceName
    deviceType
    capabilities
    buildChannel
    access
    downloadsDir
    developerMode
    debug
  }
`,N=`
  fragment ChatItemFragment on ChatItem {
    id
    fromId
    toId
    channelId
    createdAt
    content
    status
    statusData
  }
`,ee=`
  fragment SmsFragment on Sms {
    id
    body
    address
    serviceCenter
    sentAt
    type
    threadId
    subscriptionId
    isMms
    attachments {
      path
      contentType
      name
    }
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,P=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,F=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,I=`
  fragment ContactFragment on Contact {
    id
    suffix
    prefix
    firstName
    middleName
    lastName
    updatedAt
    notes
    source
    thumbnailId
    starred
    phoneNumbers {
      value
      type
      label
      normalizedNumber
    }
    addresses {
      value
      type
      label
    }
    emails {
      value
      type
      label
    }
    websites {
      value
      type
      label
    }
    events {
      value
      type
      label
    }
    ims {
      value
      protocol
      customProtocol
    }
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,L=`
  fragment CallFragment on Call {
    id
    name
    number
    durationSec
    accountId
    startedAt
    photoId
    type
    geo {
      country
      numberType
      carrier
      description
    }
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,R=`
  fragment FileFragment on File {
    isSystem
    path
    isDir
    createdAt
    updatedAt
    size
    childCount
    mediaId
  }
`,z=`
  fragment ImageFragment on Image {
    id
    title
    path
    size
    bucketId
    takenAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,te=`
  fragment VideoFragment on Video {
    id
    title
    path
    durationMs
    size
    bucketId
    createdAt
    updatedAt
    takenAt
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,ne=`
  fragment AudioFragment on Audio {
    id
    title
    artist
    path
    durationMs
    size
    bucketId
    albumFileId
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,B=`
  fragment NoteFragment on Note {
    id
    title
    content
    deletedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,re=`
  fragment DocFragment on Doc {
    id
    title
    path
    extension
    size
    bucketId
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,V=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,H=`
  fragment FeedEntryFragment on FeedEntry {
    id
    title
    url
    image
    author
    description
    content
    feedId
    rawId
    publishedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${A}
`,U=`
  fragment PackageFragment on Package {
    id
    name
    type
    version
    path
    size
    certs {
      issuer
      subject
      serialNumber
      validFrom
      validTo
    }
    installedAt
    updatedAt
  }
`,W=`
  fragment NotificationFragment on Notification {
    id
    onlyOnce
    isClearable
    appId
    appName
    postedAt
    silent
    title
    body
    actions
    replyActions
  }
`,G=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,K=`
  fragment DeviceInfoFragment on DeviceInfo {
    name
    platform
    manufacturer
    model
    osName
    osVersion
    kernelVersion
    appVersion
    appBuildNumber
    language
    cpuArch
    cpuModel
    totalMemory
    totalStorage
    display {
      width
      height
      density
    }
    android {
      sdkVersion
      versionCodeName
      securityPatch
      bootloader
      fingerprint
      hardware
      radioVersion
      board
      buildBrand
      buildNumber
      device
      javaVmVersion
      glEsVersion
      buildTime
    }
  }
`,q=`
  fragment DeviceStatusFragment on DeviceStatus {
    uptimeSec
    batteryLevel
    charging
    temperatures {
      label
      celsius
    }
    cpuUsage
    memoryAvailable
    storageAvailable
  }
`,J=`
  fragment BookmarkFragment on Bookmark {
    id
    url
    title
    faviconPath
    groupId
    pinned
    clickCount
    lastClickedAt
    sortOrder
    createdAt
    updatedAt
  }
`,Y=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,X=`
  fragment ChatChannelFragment on ChatChannel {
    actions { isOwner canRename canManageMembers canLeave canDelete }
    id
    name
    ownerId
    members {
      ...ChatChannelMemberFragment
    }
    version
    status
    createdAt
    updatedAt
  }

  fragment ChatChannelMemberFragment on ChatChannelMember {
    peerId
    status
  }

`;function Z(e){return e instanceof v?e.status===403?`remote_access_disabled`:e.message:`network_error`}function Q(e){if(e)return typeof e==`function`?e():e}function ie(e){let t=c(!1),n=c(),u=0;async function d(r,i=!1){if(e.enabled&&!e.enabled())return;let a=u;t.value=!0;try{let t=r??Q(e.variables),o=await f(typeof e.document==`function`?e.document():e.document,t,{fresh:i});if(a!==u)return;o.errors?.length?e.handle(o.data,o.errors[0].message):(n.value=o.data,e.handle(o.data,``))}catch(t){if(a!==u||t instanceof v&&t.cancelled)return;e.handle(void 0,Z(t))}finally{a===u&&(t.value=!1)}}d();let p=!0,m=l();return m&&(a(()=>{p=!1}),s(()=>{p=!0})),typeof e.variables==`function`&&r(e.variables,async()=>{await i(),p&&d()},{deep:!0}),typeof e.document==`function`&&r(e.document,async()=>{await i(),p&&d()}),e.enabled&&r(e.enabled,async(e,t)=>{await i(),p&&e&&!t&&d()}),m&&o(O.onAttached(()=>{p&&(u++,d(void 0,!0))})),{loading:t,result:n,refetch:d}}function ae(e){let t=c(!1),n=c(),r=0,i=0,u=0,d,p=0,m=!0,h;l()&&(s(()=>{m=!0}),a(()=>{m=!1}),o(O.onAttached(()=>{m&&h&&g(h.variables,{...h.options,latest:!0,force:!0})})));async function g(a,o={}){let s=o.force?++p:p;h={variables:a,options:o};let c=++r,l=o.latest?++i:void 0,m=a??Q(e.variables),g=m?{...m}:void 0,_={variables:g,requestId:c,meta:o.meta};o.latest?d=c:u++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=o.force?await f(t,g,{fresh:!0}):await f(t,g);if(s!==p||o.latest&&l!==i)return;r.errors?.length?e.handle(r.data,r.errors[0].message,_):(n.value=r.data,e.handle(r.data,``,_))}catch(t){if(s!==p||t instanceof v&&t.cancelled)return;(!o.latest||l===i)&&e.handle(void 0,Z(t),_)}finally{o.latest?d===c&&(d=void 0):u--,t.value=u>0||d!==void 0}}return{loading:t,result:n,fetch:g}}var oe=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${N}
`;`${N}`;var se=`
  query {
    peers {
      id
      name
      ip
      status
      online
      port
      deviceType
      createdAt
      updatedAt
    }
  }
`,ce=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${N}
`,le=`
  query appFiles($offset: Int!, $limit: Int!) {
    appFiles(query: "", offset: $offset, limit: $limit) {
      id
      size
      mimeType
      fileName
      createdAt
      updatedAt
    }
    appFileCount(query: "")
  }
`,ue=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,de=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${X}
`,fe=`
  query ($path: String!, $fileName: String) {
    fileInfo(path: $path, fileName: $fileName) {
      ... on FileInfo {
        path
        updatedAt
        size
      }
      data {
        ... on ImageFileInfo {
          width
          height
          location {
            latitude
            longitude
          }
        }
        ... on VideoFileInfo {
          durationMs
          width
          height
          location {
            latitude
            longitude
          }
        }
        ... on AudioFileInfo {
          durationMs
          location {
            latitude
            longitude
          }
        }
      }
    }
  }
`,pe=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${ee}
`,me=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,he=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${P}
`,ge=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${F}
`,_e=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${I}
`,ve=`
  query {
    contactSources {
      name
      type
    }
  }
`,ye=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${L}
`,be=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${z}
`,xe=`
  query {
    imageSearchStatus {
      status
      downloadProgress
      errorMessage
      modelSize
      modelDir
      isIndexing
      totalImages
      indexedImages
    }
  }
`,Se=`
  query {
    scanProgress {
      indexed
      pending
      total
      state
      error
    }
  }
`,Ce=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${te}
`,we=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${ne}
`,Te=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${R}
`,Ee=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,De=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${R}
`,Oe=`
  query {
    mounts {
      id
      name
      path
      mountPoint
      fsType
      totalBytes
      usedBytes
      freeBytes
      remote
      alias
      driveType
      diskId
      label
      diskModel
    }
  }
`,ke=`
  query {
    disks {
      id
      name
      path
      sizeBytes
      removable
      model
    }
  }
`,Ae=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,je=`
  query {
    app {
      ...AppFragment
    }
  }
  ${M}
`,Me=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Ne=`
  query audioQueue($offset: Int!, $limit: Int!) {
    items: audioQueueItems(query: "", offset: $offset, limit: $limit) {
      ...AudioItemFragment
    }
    total: audioQueueItemCount
    playback: audioPlayback {
      revision
      mode
      mediaId
      path
      isPlaying
      positionMs
    }
  }
  ${j}
`,Pe=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${k}
`,Fe=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,Ie=`
  query notes($offset: Int!, $limit: Int!, $query: String!) {
    notes(query: $query, offset: $offset, limit: $limit) {
      id
      title
      deletedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    noteCount(query: $query)
  }
  ${A}
`,Le=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${B}
`,Re=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${V}
`,ze=`
  query feedEntries($offset: Int!, $limit: Int!, $query: String!) {
    items: feedEntries(query: $query, offset: $offset, limit: $limit) {
      id
      title
      url
      image
      author
      feedId
      rawId
      publishedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    total: feedEntryCount(query: $query)
  }
  ${A}
`,Be=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${V}
  ${k}
`,Ve=`
  query bucketsTags($type: MediaDataType!, $tagType: DataType!) {
    tags(type: $tagType) {
      ...TagFragment
    }
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
  ${k}
`,He=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${V}
  ${H}
`,Ue=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,We=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Ge=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${re}
`,Ke=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,qe=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Je=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,Ye=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      id
      count
    }
  }
`,Xe=`
  query {
    total: contactCount(query: "")
  }
`,Ze=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,Qe=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,$e=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${P}
`,et=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${F}
`,tt=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,nt=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${U}
`,rt=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,it=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,at=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,ot=`
  query {
    screenMirrorControlEnabled
  }
`,st=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,ct=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${W}
`,lt=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${G}
`,ut=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${K}
  ${q}
`,dt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,ft=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,pt=`
  query {
    appLogPath
  }
`,mt=`
  query {
    dbPath
  }
`,ht=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId)
  }
`,gt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,_t=`
  query {
    pomodoroToday {
      date
      completedCount
      currentRound
      timeLeftSec
      totalTimeSec
      isRunning
      isPaused
      state
    }
    pomodoroSettings {
      workDurationMin
      shortBreakDurationMin
      longBreakDurationMin
      pomodorosBeforeLongBreak
      showNotification
      playSoundOnComplete
    }
  }
`,vt=`
  query {
    prefs
  }
`,yt=`
  query {
    dbTables
  }
`,$=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,bt=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,xt=`
  query DbTableInfo($table: String!) {
    dbTableInfo(table: $table) {
      createSql
      idKey
    }
  }
`,St=`
  query DbTableColumns($table: String!) {
    dbTableColumns(table: $table) {
      name
      dataType
      notNull
      defaultValue
      primaryKey
    }
  }
`,Ct=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${J}
  ${Y}
`,wt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,Tt=`
  query NearbyDevices {
    nearbyDevices { id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    pairingStates { deviceId generation mode phase error verificationCode }
    mdnsState { scanning }
  }
`,Et=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${R}
`,Dt=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${R}
`;export{Oe as $,Ee as A,N as At,fe as B,T as Bt,St as C,ht as Ct,yt as D,J as Dt,bt as E,j as Et,ze as F,R as Ft,be as G,Te as H,Ye as I,B as It,ie as J,Et as K,He as L,k as Lt,Ke as M,I as Mt,Ge as N,H as Nt,ut as O,Y as Ot,Ae as P,V as Pt,gt as Q,Re as R,O as Rt,mt as S,Pe as St,$ as T,Ce as Tt,Ue as U,Dt as V,xe as W,Fe as X,it as Y,Me as Z,oe as _,he as _t,wt as a,Je as at,ve as b,pe as bt,We as c,se as ct,Ct as d,De as dt,Tt as et,Ve as f,st as ft,ce as g,me as gt,de as h,at as ht,ft as i,ct as it,ke as j,G as jt,dt as k,X as kt,Ne as l,_t as lt,ye as m,ot as mt,je as n,Le as nt,$e as o,rt as ot,Ze as p,Se as pt,ae as q,pt as r,Ie as rt,et as s,nt as st,le as t,tt,we as u,vt as ut,lt as v,ge as vt,xt as w,qe as wt,_e as x,ue as xt,Xe as y,Qe as yt,Be as z,E as zt};