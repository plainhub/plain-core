import{_ as e}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{t}from"./chunk-eventbus-Ca-q0d_H.js";import{a as n,t as r}from"./chunk-gql-client-Cy6q4k67.js";import{At as i,Dt as a,Ft as o,It as s,Lt as c,Nt as l,Ot as u,Pt as d,Rt as f,jt as p,kt as m}from"./chunk-query-Ctn1STag.js";function ee(i,a=!0){let o=e(!1),s=[],c=[];async function l(e){o.value=!0;try{let o=await n(i.document,e,{dedupe:!1});if(o.errors?.length){let e=o.errors[0].message;a&&t.emit(`toast`,e);let n=new r(e);for(let e of c)e(n);return}for(let e of s)e(o);return o}catch(e){if(e instanceof r&&e.cancelled)return;let n=e instanceof r?e.message:`network_error`;a&&t.emit(`toast`,n);for(let t of c)t(e);return}finally{o.value=!1}}function u(e){return s.push(e),{off:()=>{let t=s.indexOf(e);t>=0&&s.splice(t,1)}}}function d(e){return c.push(e),{off:()=>{let t=c.indexOf(e);t>=0&&c.splice(t,1)}}}return{mutate:l,loading:o,onDone:u,onError:d}}async function te(e,t){return await e(t)!=null}var h=`
  mutation {
    clearAppLogs
  }
`,g=`
  mutation setPref($key: String!, $value: JSON!) {
    setPref(key: $key, value: $value)
  }
`,_=`
  mutation updateDeviceName($name: String!) {
    updateDeviceName(name: $name)
  }
`,v=`
  mutation forwardChatItems($id: ID!, $targets: [String!]!) {
    forwardChatItems(id: $id, targets: $targets) {
      ...ChatItemFragment
    }
  }
  ${p}
`,y=`
  mutation sendChatItem($target: String!, $content: String!) {
    sendChatItem(target: $target, content: $content) {
      ...ChatItemFragment
    }
  }
  ${p}
`,b=`
  mutation deleteChatItem($id: ID!) {
    deleteChatItem(id: $id)
  }
`,x=`
  mutation deleteChatItems($query: String!) {
    deleteChatItems(query: $query) {
      affectedCount
    }
  }
`,S=`
  mutation retryChatItem($id: ID!) {
    retryChatItem(id: $id) {
      ...ChatItemFragment
    }
  }
  ${p}
`,C=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${i}
`,w=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${i}
`,T=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,E=`
  mutation deletePeer($id: ID!) {
    deletePeer(id: $id)
  }
`,D=`
  mutation unpairPeer($id: ID!) {
    unpairPeer(id: $id)
  }
`,O=`
  mutation leaveChatChannel($id: ID!) {
    leaveChatChannel(id: $id)
  }
`,k=`
  mutation addChatChannelMember($id: ID!, $peerId: ID!) {
    addChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${i}
`,A=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${i}
`,j=`
  mutation acceptChatChannelInvite($id: ID!) {
    acceptChatChannelInvite(id: $id)
  }
`,M=`
  mutation declineChatChannelInvite($id: ID!) {
    declineChatChannelInvite(id: $id)
  }
`,N=`
  mutation createDir($path: String!) {
    createDir(path: $path) {
      ...FileFragment
    }
  }
  ${s}
`,P=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${s}
`,F=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,I=`
  mutation copyFile($src: String!, $dst: String!, $overwrite: Boolean!) {
    copyFile(src: $src, dst: $dst, overwrite: $overwrite)
  }
`,L=`
  mutation createFileTask($type: FileTaskType!, $title: String!, $ops: [FileTaskOpInput!]!) {
    createFileTask(type: $type, title: $title, ops: $ops, waitForCompletion: true) {
      status
      error
    }
  }
`,R=`
  mutation recordMediaPlay($mediaId: ID!, $type: MediaDataType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,z=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: MediaDataType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation playAudio($mediaId: ID!) {
    playAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,V=`
  mutation enqueueAndPlayAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    playAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,H=`
  mutation updateAudioPlayMode($mode: MediaPlayMode!) {
    updateAudioPlayMode(mode: $mode)
  }
`,U=`
  mutation removeAudioFromQueue($mediaId: ID!) {
    removeAudioFromQueue(mediaId: $mediaId)
  }
`,W=`
  mutation addAudioToQueue($query: String!) {
    addAudioToQueue(query: $query)
  }
`,G=`
  mutation clearAudioQueue {
    clearAudioQueue
  }
`,K=`
  mutation reorderAudioQueue($mediaIds: [ID!]!) {
    reorderAudioQueue(mediaIds: $mediaIds)
  }
`,q=`
  mutation deleteMediaItems($type: MediaDataType!, $query: String!) {
    deleteMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,J=`
  mutation trashMediaItems($type: MediaDataType!, $query: String!) {
    trashMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Y=`
  mutation restoreMediaItems($type: MediaDataType!, $query: String!) {
    restoreMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,X=`
  mutation moveMediaItems($type: MediaDataType!, $query: String!, $destDir: String!) {
    moveMediaItems(type: $type, query: $query, destDir: $destDir) {
      affectedCount
    }
  }
`,Z=`
  mutation trashSms($query: String!) {
    trashSms(query: $query) {
      affectedCount
    }
  }
`,Q=`
  mutation restoreSms($query: String!) {
    restoreSms(query: $query) {
      affectedCount
    }
  }
`,ne=`
  mutation deleteSms($query: String!) {
    deleteSms(query: $query) {
      affectedCount
    }
  }
`,re=`
  mutation removeFromTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    removeFromTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ie=`
  mutation addToTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    addToTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ae=`
  mutation updateTagRelations($type: DataType!, $keys: [String!]!, $addTagIds: [ID!]!, $removeTagIds: [ID!]!) {
    updateTagRelations(type: $type, keys: $keys, addTagIds: $addTagIds, removeTagIds: $removeTagIds)
  }
`,oe=`
  mutation createTag($type: DataType!, $name: String!) {
    createTag(type: $type, name: $name) {
      ...TagFragment
    }
  }
  ${f}
`,se=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${f}
`,ce=`
  mutation deleteTag($id: ID!) {
    deleteTag(id: $id)
  }
`,le=`
  mutation addFavoriteFolder($rootPath: String!, $fullPath: String!) {
    addFavoriteFolder(rootPath: $rootPath, fullPath: $fullPath) {
      rootPath
      fullPath
    }
  }
`,ue=`
  mutation removeFavoriteFolder($fullPath: String!) {
    removeFavoriteFolder(fullPath: $fullPath) {
      rootPath
      fullPath
      alias
    }
  }
`,de=`
  mutation setFavoriteFolderAlias($fullPath: String!, $alias: String!) {
    setFavoriteFolderAlias(fullPath: $fullPath, alias: $alias) {
      rootPath
      fullPath
      alias
    }
  }
`,fe=`
  mutation createNote($input: NoteInput!) {
    createNote(input: $input) {
      ...NoteFragment
    }
  }
  ${c}
`,pe=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${c}
`,me=`
  mutation deleteNotes($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,he=`
  mutation trashNotes($query: String!) {
    trashNotes(query: $query) {
      affectedCount
    }
  }
`,ge=`
  mutation restoreNotes($query: String!) {
    restoreNotes(query: $query) {
      affectedCount
    }
  }
`,_e=`
  mutation deleteFeedEntries($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,ve=`
  mutation deleteCalls($query: String!) {
    deleteCalls(query: $query) {
      affectedCount
    }
  }
`,ye=`
  mutation deleteContacts($query: String!) {
    deleteContacts(query: $query) {
      affectedCount
    }
  }
`,be=`
  mutation createFeed($url: String!, $fetchContent: Boolean!) {
    createFeed(url: $url, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${o}
`,xe=`
  mutation importFeeds($content: String!) {
    importFeeds(content: $content)
  }
`,Se=`
  mutation exportFeeds {
    exportFeeds
  }
`,Ce=`
  mutation exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`,we=`
  mutation relaunchApp {
    relaunchApp
  }
`,Te=`
  mutation openAccessibilitySettings {
    openAccessibilitySettings
  }
`,Ee=`
  mutation requestAccess($feature: AccessFeature!) {
    requestAccess(feature: $feature)
  }
`,De=`
  mutation deleteFeed($id: ID!) {
    deleteFeed(id: $id)
  }
`,Oe=`
  mutation updateFeed($id: ID!, $name: String!, $fetchContent: Boolean!) {
    updateFeed(id: $id, name: $name, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${o}
`,ke=`
  mutation syncFeeds($id: ID) {
    syncFeeds(id: $id)
  }
`,Ae=`
  mutation syncFeedEntryContent($id: ID!) {
    syncFeedEntryContent(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${o}
  ${d}
`,je=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,Me=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Ne=`
  mutation sendSms($number: String!, $body: String!, $subscriptionId: Int!) {
    sendSms(number: $number, body: $body, subscriptionId: $subscriptionId)
  }
`,Pe=`
  mutation sendSms($number: String!, $body: String!, $subscriptionId: Int!, $requestId: String!) {
    sendSms(number: $number, body: $body, subscriptionId: $subscriptionId, requestId: $requestId)
  }
`,Fe=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Ie=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Le=`
  mutation sendMms($number: String!, $body: String!, $attachmentPaths: [String!]!, $threadId: ID!) {
    sendMms(number: $number, body: $body, attachmentPaths: $attachmentPaths, threadId: $threadId)
  }
`,Re=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,ze=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Be=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,Ve=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,He=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Ue=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,We=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,Ge=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,Ke=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,qe=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Je=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,Ye=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Xe=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Ze=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Qe=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec)
  }
`,$e=`
  mutation stopPomodoro {
    stopPomodoro
  }
`,et=`
  mutation pausePomodoro {
    pausePomodoro
  }
`,tt=`mutation logout { logout }`,nt=`
  mutation importBookmarks($content: String!) {
    importBookmarks(content: $content) { affectedCount }
  }
`,rt=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${u}
`,it=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${u}
`,at=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,ot=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,st=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${m}
`,ct=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${m}
`,lt=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,ut=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,dt=`
  mutation { importImageSearchModel }
`,ft=`
  mutation { enableImageSearch }
`,pt=`
  mutation { disableImageSearch }
`,mt=`
  mutation { cancelImageModelDownload }
`,ht=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,gt=`
  mutation { cancelImageIndex }
`,_t=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${l}
`,vt=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${l}
`,yt=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,bt=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,xt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,St=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,Ct=`
  mutation AcquireDiscovery($token: ID!) {
    acquireDiscovery(token: $token)
  }
`,wt=`
  mutation ReleaseDiscovery($token: ID!) {
    releaseDiscovery(token: $token)
  }
`,Tt=`
  mutation pairDevice($input: PairingDeviceInput!) {
    pairDevice(input: $input) { deviceId generation mode phase error }
  }
`,Et=`
  mutation cancelPairing($generation: String!) {
    cancelPairing(generation: $generation)
  }
`,$=`
  mutation respondToPairing($input: PairingRequestInput!, $accepted: Boolean!) {
    respondToPairing(input: $input, accepted: $accepted)
  }
`,Dt=`
  mutation downloadPeerFile($messageId: ID!, $peerId: ID!) {
    downloadPeerFile(messageId: $messageId, peerId: $peerId)
  }
`,Ot=`
  mutation pauseDownload($messageId: ID!) {
    pauseDownload(messageId: $messageId)
  }
`,kt=`
  mutation resumeDownload($messageId: ID!, $peerId: ID!) {
    resumeDownload(messageId: $messageId, peerId: $peerId)
  }
`,At=`
  mutation retryDownload($messageId: ID!, $peerId: ID!) {
    retryDownload(messageId: $messageId, peerId: $peerId)
  }
`,jt=`
  mutation pauseMediaScan { pauseMediaScan }
`,Mt=`
  mutation resumeMediaScan { resumeMediaScan }
`,Nt=`
  mutation stopMediaScan { stopMediaScan }
`,Pt=`
  mutation rebuildMediaIndex($root: String!) { rebuildMediaIndex(root: $root) }
`,Ft=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`,It=`
  mutation advanceAudioSelection($action: AudioSelectionAction!, $expectedPath: String!, $expectedRevision: Long!) {
    advanceAudioSelection(action: $action, expectedPath: $expectedPath, expectedRevision: $expectedRevision) {
      track { ...AudioItemFragment }
      path
      positionMs
      revision
    }
  }
  ${a}
`;export{Ft as $,Be as $t,b as A,Ee as At,q as B,te as Bt,fe as C,A as Ct,at as D,F as Dt,lt as E,xt as Et,St as F,Q as Ft,ne as G,Ne as Gt,me as H,z as Ht,_e as I,kt as It,Dt as J,de as Jt,ce as K,Pe as Kt,bt as L,Mt as Lt,Ze as M,$ as Mt,Ke as N,Y as Nt,ve as O,K as Ot,ye as P,ge as Pt,Ce as Q,Qe as Qt,De as R,S as Rt,L as S,P as Sn,U as St,M as T,re as Tt,We as U,y as Ut,yt as V,Je as Vt,E as W,Le as Wt,V as X,Ue as Xt,ft as Y,g as Yt,Se as Z,ht as Zt,st as _,Oe as _n,Pt as _t,k as a,J as an,ze as at,N as b,se as bn,we as bt,It as c,Ie as cn,Xe as ct,gt as d,H as dn,Te as dt,Nt as en,v as et,mt as f,it as fn,Tt as ft,I as g,_ as gn,B as gt,G as h,vt as hn,et as ht,rt as i,ke as in,ee as it,x as j,Ve as jt,T as k,Ge as kt,Fe as l,Re as ln,Ye as lt,h as m,w as mn,jt as mt,Ct as n,He as nn,xe as nt,le as o,he as on,O as ot,Et as p,ct as pn,Ot as pt,pt as q,Me as qt,W as r,Ae as rn,dt as rt,ie as s,Z as sn,tt as st,j as t,$e as tn,nt as tt,je as u,D as un,X as ut,C as v,pe as vn,ot as vt,oe as w,ue as wt,be as x,ae as xn,wt as xt,_t as y,qe as yn,R as yt,ut as z,At as zt};