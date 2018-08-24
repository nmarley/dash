// Copyright (c) 2014-2017 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#ifndef SPORK_H
#define SPORK_H

#include "hash.h"
#include "net.h"
#include "utilstrencodings.h"
#include "key.h"

class CSporkMessage;
class CSporkManager;

static const int DEFAULT_SPORK_SIG_THRESHOLD = 2;  // for M of N sporks

/*
    Don't ever reuse these IDs for other sporks
    - This would result in old clients getting confused about which spork is for what
*/
static const int SPORK_2_INSTANTSEND_ENABLED                            = 10001;
static const int SPORK_3_INSTANTSEND_BLOCK_FILTERING                    = 10002;
static const int SPORK_5_INSTANTSEND_MAX_VALUE                          = 10004;
static const int SPORK_6_NEW_SIGS                                       = 10005;
static const int SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT                 = 10007;
static const int SPORK_9_SUPERBLOCKS_ENABLED                            = 10008;
static const int SPORK_10_MASTERNODE_PAY_UPDATED_NODES                  = 10009;
static const int SPORK_12_RECONSIDER_BLOCKS                             = 10011;
static const int SPORK_14_REQUIRE_SENTINEL_FLAG                         = 10013;

static const int SPORK_START                                            = SPORK_2_INSTANTSEND_ENABLED;
static const int SPORK_END                                              = SPORK_14_REQUIRE_SENTINEL_FLAG;

extern std::map<int, int64_t> mapSporkDefaults;
// extern std::map<uint256, CSporkMessage> mapSporks;
extern std::map<uint256, CSporkMessage> mapSporks;
extern CSporkManager sporkManager;

// CSporkAddr is a pair of string, hash160 where string is the signer ID.
using CSporkAddr = std::pair<std::string, CKeyID>;

//
// Spork classes
// Keep track of all of the network spork settings
//

class CSporkMessage
{
private:
    std::vector<unsigned char> vchSig;

public:
    int nSporkID;
    int64_t nValue;
    int64_t nTimeSigned;

    CSporkMessage(int nSporkID, int64_t nValue, int64_t nTimeSigned) :
        nSporkID(nSporkID),
        nValue(nValue),
        nTimeSigned(nTimeSigned)
        {}

    CSporkMessage() :
        nSporkID(0),
        nValue(0),
        nTimeSigned(0)
        {}


    ADD_SERIALIZE_METHODS;

    template <typename Stream, typename Operation>
    inline void SerializationOp(Stream& s, Operation ser_action) {
        READWRITE(nSporkID);
        READWRITE(nValue);
        READWRITE(nTimeSigned);
        if (!(s.GetType() & SER_GETHASH)) {
            READWRITE(vchSig);
        }
    }

    uint256 GetHash() const;
    uint256 GetSignatureHash() const;

    bool Sign(const CKey& key, bool fSporkSixActive);
    bool CheckSignature(const CKeyID& pubKeyId, bool fSporkSixActive) const;
    void Relay(CConnman& connman);
};


class CSporkManager
{
private:
    mutable CCriticalSection cs;
    std::map<uint256, CSporkMessage> mapSporksByHash;

    // Map of all active, signed & verified sporks.
    // Spork ID (internal ID) : Wire spork message.
    std::map<int, CSporkMessage> mapSporksActive;

    // CKeyID legacySporkPubKeyID;

    // using CSporkAddr = std::pair<std::string, CKeyID>
    // std::pair<std::string, CKey> sporkPrivKeyPair; // signerID, privkey

    std::string sporkSignerID;
    CKey sporkPrivKey;

//    std::map<CKeyID, int> mapSporkKeyIDs;
    std::vector<CKeyID> vecSporkKeyIDs;

public:

    CSporkManager() {}

    ADD_SERIALIZE_METHODS;

    template <typename Stream, typename Operation>
    inline void SerializationOp(Stream& s, Operation ser_action) {
        // TODO: clear spork keys upon initialization, ignore these...
        READWRITE(vecSporkKeyIDs);
        READWRITE(mapSporksByHash);
        READWRITE(mapSporksActive);
        // we don't serialize private key to prevent its leakage
    }

    void Clear();
    /// Dummy implementation for CFlatDB
    void CheckAndRemove() {}

    void ProcessSpork(CNode* pfrom, const std::string& strCommand, CDataStream& vRecv, CConnman& connman);
    void ExecuteSpork(int nSporkID, int nValue);
    bool UpdateSpork(int nSporkID, int64_t nValue, CConnman& connman);

    bool IsSporkActive(int nSporkID);
    int64_t GetSporkValue(int nSporkID);
    int GetSporkIDByName(const std::string& strName);
    std::string GetSporkNameByID(int nSporkID);

    bool GetSporkByHash(const uint256& hash, CSporkMessage &sporkRet);

    bool SetSporkAddress(const std::string& strAddress);
    bool SetPrivKey(const std::string& strPrivKey);

    std::string ToString() const;
};

#endif
