// Copyright (c) 2014-2018 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.
#ifndef GOVERNANCE_CLASSES_H
#define GOVERNANCE_CLASSES_H

#include "base58.h"
#include "governance.h"
#include "key.h"
#include "script/standard.h"
#include "util.h"

class CSuperblock;
class CGovernanceTriggerManager;
class CSuperblockManager;

typedef std::shared_ptr<CSuperblock> CSuperblock_sptr;

// DECLARE GLOBAL VARIABLES FOR GOVERNANCE CLASSES
extern CGovernanceTriggerManager triggerman;

/**
*   Trigger Mananger
*
*   - Track governance objects which are triggers
*   - After triggers are activated and executed, they can be removed
*/

class CGovernanceTriggerManager
{
    friend class CSuperblockManager;
    friend class CGovernanceManager;

private:
    std::map<uint256, CSuperblock_sptr> mapTrigger;

    std::vector<CSuperblock_sptr> GetActiveTriggers();
    bool AddNewTrigger(uint256 nHash);
    void CleanAndRemove();

public:
    CGovernanceTriggerManager() :
        mapTrigger() {}
};

/**
*   Superblock Manager
*
*   Class for querying superblock information
*/

class CSuperblockManager
{
private:
    static bool GetBestSuperblock(CSuperblock_sptr& pSuperblockRet, int nBlockHeight);

public:
    static bool IsSuperblockTriggered(int nBlockHeight);

    static bool GetSuperblockPayments(int nBlockHeight, std::vector<CTxOut>& voutSuperblockRet);
    static void ExecuteBestSuperblock(int nBlockHeight);

    static std::string GetRequiredPaymentsString(int nBlockHeight);
    static bool IsValid(const CTransaction& txNew, int nBlockHeight, CAmount blockReward);
};

/**
*   Governance Object Payment
*
*/

class CGovernancePayment
{
private:
    bool fValid;

public:
    CScript script;
    CAmount nAmount;

    CGovernancePayment() :
        fValid(false),
        script(),
        nAmount(0)
    {
    }

    CGovernancePayment(CBitcoinAddress addrIn, CAmount nAmountIn) :
        fValid(false),
        script(),
        nAmount(0)
    {
        try {
            CTxDestination dest = addrIn.Get();
            script = GetScriptForDestination(dest);
            nAmount = nAmountIn;
            fValid = true;
        } catch (std::exception& e) {
            LogPrintf("CGovernancePayment Payment not valid: addrIn = %s, nAmountIn = %d, what = %s\n",
                addrIn.ToString(), nAmountIn, e.what());
        } catch (...) {
            LogPrintf("CGovernancePayment Payment not valid: addrIn = %s, nAmountIn = %d\n",
                addrIn.ToString(), nAmountIn);
        }
    }

    bool IsValid() { return fValid; }
};


/**
*   Trigger : Superblock
*
*   - Create payments on the network
*
*   object structure:
*   {
*       "governance_object_id" : last_id,
*       "type" : govtypes.trigger,
*       "subtype" : "superblock",
*       "superblock_name" : superblock_name,
*       "start_epoch" : start_epoch,
*       "payment_addresses" : "addr1|addr2|addr3",
*       "payment_amounts"   : "amount1|amount2|amount3"
*   }
*/

class CSuperblock : public CGovernanceObject
{
private:
    uint256 nGovObjHash;

    int nBlockHeight;
    int nStatus;
    std::vector<CGovernancePayment> vecPayments;

    void ParsePaymentSchedule(const std::string& strPaymentAddresses, const std::string& strPaymentAmounts);

public:
    CSuperblock();
    CSuperblock(uint256& nHash);

    static bool IsValidBlockHeight(int nBlockHeight);
    static void GetNearestSuperblocksHeights(int nBlockHeight, int& nLastSuperblockRet, int& nNextSuperblockRet);
    static CAmount GetPaymentsLimit(int nBlockHeight);

    int GetStatus() { return nStatus; }
    void SetStatus(int nStatusIn) { nStatus = nStatusIn; }

    // IS THIS TRIGGER ALREADY EXECUTED?
    bool IsExecuted() { return nStatus == SEEN_OBJECT_EXECUTED; }
    // TELL THE ENGINE WE EXECUTED THIS EVENT
    void SetExecuted() { nStatus = SEEN_OBJECT_EXECUTED; }

    CGovernanceObject* GetGovernanceObject()
    {
        AssertLockHeld(governance.cs);
        CGovernanceObject* pObj = governance.FindGovernanceObject(nGovObjHash);
        return pObj;
    }

    int GetBlockHeight()
    {
        return nBlockHeight;
    }

    int CountPayments() { return (int)vecPayments.size(); }
    bool GetPayment(int nPaymentIndex, CGovernancePayment& paymentRet);
    CAmount GetPaymentsTotalAmount();

    bool IsValid(const CTransaction& txNew, int nBlockHeight, CAmount blockReward);
    bool IsExpired();
};

class CProposalDetail {
private:
    // Payload data members
    std::string strName;
    std::string strURL;
    int nStartEpoch;
    int nEndEpoch;
    CAmount nPaymentAmount;
    CBitcoinAddress payeeAddr;

    // Parsing related
    std::vector<std::string> vecStrErrMessages;
    bool fParsedOK;
    void ParseStrDataHex(const std::string& strDataHex);

public:
    explicit CProposalDetail(const std::string& strDataHex);

    // Parsing
    std::string ErrorMessages() const;
    bool DidParse() const { return fParsedOK; }

    // Only implemented these b/c this is all that's necessary... we can
    // implement the other accessors later if needed.
    std::string Name() const { return strName; }
    CAmount Amount() const { return nPaymentAmount; }
    CBitcoinAddress Address() const { return payeeAddr; }

    uint256 GetHash() const;
};

// CPayment represents a Dash superblock payment for a single proposal.
struct CPayment {
//    CPayment();

    CPayment(const uint256& nProposalHash, CBitcoinAddress address, CAmount nAmount);

//    CPayment(const CPayment& other);

//    CPayment& operator=(const CPayment& ref) = default;
//
//    CPayment(CPayment&& ref)
//    {
//        std::swap(nProposalHash, ref.nProposalHash);
//        std::swap(address, ref.address);
//        std::swap(nAmount, ref.nAmount);
//    }
//    CPayment& operator=(CPayment&& ref)
//    {
//        std::swap(nProposalHash, ref.nProposalHash);
//        std::swap(address, ref.address);
//        std::swap(nAmount, ref.nAmount);
//        return *this;
//    }

    uint256 nProposalHash;
    CBitcoinAddress address;
    CAmount nAmount;

    bool operator<(const CPayment& other) const
    {
        return (UintToArith256(nProposalHash) < UintToArith256(other.nProposalHash));
    }

    bool operator==(const CPayment& other) const
    {
        return (
            (nProposalHash == other.nProposalHash) &&
            (address == other.address) &&
            (nAmount == other.nAmount)
        );
    }

    ADD_SERIALIZE_METHODS;

    template <typename Stream, typename Operation>
    inline void SerializationOp(Stream& s, Operation ser_action)
    {
        READWRITE(nProposalHash);
        READWRITE(address.ToString());
        READWRITE(nAmount);
    }

//    friend std::ostream& operator<<(std::ostream& os, const CPayment& p)
//    {
//        os << p.proposalHash.ToString();
//        os << p.addr.ToString();
//        os << p.amt;
//
//        return os;
//    }
};

class CTriggerDetail {
private:
    // Payload data members
    int nHeight;
    std::vector<CPayment> vecPayments;

    // Parsing related
    std::vector<std::string> vecStrErrMessages;
    bool fParsedOK;
    void ParseStrDataHex(const std::string& strDataHex);

public:
    CTriggerDetail();

    explicit CTriggerDetail(const std::string& strDataHex);

    CTriggerDetail(int nHeight, const std::vector<const CGovernanceObject *>& vecProposals);

    std::string GetDataHexStr() const;
    uint256 GetHash();

    // Parsing
    std::string ErrorMessages() const;
    bool DidParse() const { return fParsedOK; }
};

#endif
