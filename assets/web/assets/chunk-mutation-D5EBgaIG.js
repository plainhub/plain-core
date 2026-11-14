import{_ as e}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{t}from"./chunk-eventbus-Ca-q0d_H.js";import{a as n,t as r}from"./chunk-gql-client-CX6c6WBf.js";import{Dt as i,Et as a,Ft as o,It as s,Mt as c,Nt as l,Ot as u,Pt as d,Tt as f,jt as p,kt as m}from"./chunk-query-OYL26kY9.js";function ee(i,a=!0){let o=e(!1),s=[],c=[];async function l(e){o.value=!0;try{let o=await n(i.document,e,{dedupe:!1});if(o.errors?.length){let e=o.errors[0].message;a&&t.emit(`toast`,e);let n=new r(e);for(let e of c)e(n);return}for(let e of s)e(o);return o}catch(e){if(e instanceof r&&e.cancelled)return;let n=e instanceof r?e.message:`network_error`;a&&t.emit(`toast`,n);for(let t of c)t(e);return}finally{o.value=!1}}function u(e){return s.push(e),{off:()=>{let t=s.indexOf(e);t>=0&&s.splice(t,1)}}}function d(e){return c.push(e),{off:()=>{let t=c.indexOf(e);t>=0&&c.splice(t,1)}}}return{mutate:l,loading:o,onDone:u,onError:d}}async function te(e,t){return await e(t)!=null}var h=`
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
  ${m}
`,y=`
  mutation sendChatItems($targets: [String!]!, $content: String!) {
    sendChatItems(targets: $targets, content: $content) {
      ...ChatItemFragment
    }
  }
  ${m}
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
  ${m}
`,C=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${u}
`,w=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${u}
`,T=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,E=`
  mutation deletePeer($peerId: ID!) {
    deletePeer(peerId: $peerId)
  }
`,D=`
  mutation unpairPeer($peerId: ID!) {
    unpairPeer(peerId: $peerId)
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
  ${u}
`,A=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${u}
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
  ${d}
`,P=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${d}
`,F=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,I=`
  mutation copyFileToDirectory($src: String!, $directory: String!, $name: String!) {
    copyFileToDirectory(src: $src, directory: $directory, name: $name)
  }
`,L=`
  mutation createFileTask($type: FileTaskType!, $title: String!, $ops: [FileTaskOpInput!]!) {
    createFileTask(type: $type, title: $title, ops: $ops, waitForCompletion: true) {
      status
      error
    }
  }
`,R=`
  mutation recordMediaPlay($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,z=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation selectAudio($mediaId: ID!) {
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${f}
`,V=`
  mutation enqueueAndSelectAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${f}
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
  ${s}
`,se=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${s}
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
  ${o}
`,pe=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${o}
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
  ${l}
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
  ${l}
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
  ${l}
  ${c}
`,je=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,Me=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Ne=`
  mutation sendSms($number: String!, $body: String!, $attachmentPaths: [String!]! = [], $subscriptionId: Int, $requestId: String!) {
    sendSms(number: $number, body: $body, attachmentPaths: $attachmentPaths, subscriptionId: $subscriptionId, requestId: $requestId) {
      requestId
      requiresConfirmation
    }
  }
`,Pe=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Fe=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Ie=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,Le=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Re=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,ze=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,Be=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Ve=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,He=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,Ue=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,We=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,Ge=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Ke=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,qe=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Je=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Ye=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Xe=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec)
  }
`,Ze=`
  mutation stopPomodoro {
    stopPomodoro
  }
`,Qe=`
  mutation pausePomodoro {
    pausePomodoro
  }
`,$e=`mutation logout { logout }`,et=`
  mutation importBookmarks($content: String!) {
    importBookmarks(content: $content) { affectedCount }
  }
`,tt=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${a}
`,nt=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${a}
`,rt=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,it=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,at=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${i}
`,ot=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${i}
`,st=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,ct=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,lt=`
  mutation { importImageSearchModel }
`,ut=`
  mutation { enableImageSearch }
`,dt=`
  mutation { disableImageSearch }
`,ft=`
  mutation { cancelImageModelDownload }
`,pt=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,mt=`
  mutation { cancelImageIndex }
`,ht=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${p}
`,gt=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${p}
`,_t=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,vt=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,yt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,$=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,bt=`
  mutation StartDiscovery {
    startDiscovery
  }
`,xt=`
  mutation StopDiscovery {
    stopDiscovery
  }
`,St=`
  mutation PairDevice($input: PairPeerInput!) {
    pairPeer(input: $input)
  }
`,Ct=`
  mutation cancelPairing($peerId: ID!) {
    cancelPairing(peerId: $peerId)
  }
`,wt=`
  mutation respondToPairing($peerId: ID!, $accepted: Boolean!) {
    respondToPairing(peerId: $peerId, accepted: $accepted)
  }
`,Tt=`
  mutation createDownload($input: DownloadInput!) {
    createDownload(input: $input) { id }
  }
`,Et=`
  mutation pauseDownload($id: ID!) { pauseDownload(id: $id) }
`,Dt=`
  mutation resumeDownload($id: ID!) { resumeDownload(id: $id) }
`,Ot=`
  mutation retryDownload($id: ID!) { retryDownload(id: $id) }
`,kt=`
  mutation pauseMediaScan { pauseMediaScan }
`,At=`
  mutation resumeMediaScan { resumeMediaScan }
`,jt=`
  mutation stopMediaScan { stopMediaScan }
`,Mt=`
  mutation rebuildMediaIndex($root: String!) { rebuildMediaIndex(root: $root) }
`,Nt=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`;export{et as $,Be as $t,x as A,ge as At,_t as B,y as Bt,oe as C,F as Ct,ve as D,ze as Dt,rt as E,Ee as Et,_e as F,Ot as Ft,ce as G,Ve as Gt,He as H,Me as Ht,vt as I,te as It,V as J,Xe as Jt,dt as K,bt as Kt,De as L,Ke as Lt,We as M,Dt as Mt,ye as N,At as Nt,T as O,wt as Ot,$ as P,S as Pt,v as Q,Ze as Qt,ct as R,z as Rt,fe as S,yt as St,st as T,Ue as Tt,E as U,de as Ut,me as V,Ne as Vt,ne as W,g as Wt,Ce as X,xt as Xt,Se as Y,Re as Yt,Nt as Z,jt as Zt,ht as _,se as _n,we as _t,le as a,Fe as an,$e as at,be as b,ue as bt,je as c,H as cn,X as ct,Ct as d,w as dn,Et as dt,Ae as en,xe as et,h as f,gt as fn,kt as ft,C as g,Ge as gn,R as gt,at as h,pe as hn,it as ht,k as i,Z as in,O as it,Ye as j,Q as jt,b as k,Y as kt,mt as l,nt as ln,Te as lt,I as m,Oe as mn,Mt as mt,W as n,J as nn,ee as nt,ie as o,Ie as on,Je as ot,G as p,_ as pn,Qe as pt,ut as q,pt as qt,tt as r,he as rn,Le as rt,Pe as s,D as sn,qe as st,j as t,ke as tn,lt as tt,ft as u,ot as un,St as ut,N as v,ae as vn,U as vt,M as w,K as wt,L as x,re as xt,Tt as y,P as yn,A as yt,q as z,B as zt};