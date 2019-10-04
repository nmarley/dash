// Copyright (c) 2019 The Dash Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#ifndef DASH_PROPOSALTX_H
#define DASH_PROPOSALTX_H

#include "consensus/validation.h"
#include "primitives/transaction.h"

#include "base58.h"
#include "pubkey.h"
#include "univalue.h"

class CBlock;
class CBlockIndex;

/** Proposal types */
enum class PropTxType {
    Funding = 1,
    GovChange = 2,
};
// PropTxType::Funding
// PropTxType::GovChange


// Transaction with governance proposal payload
class CProposalTx
{
public:
    static const uint16_t CURRENT_VERSION = 1;

    uint16_t nVersion{CURRENT_VERSION};
    uint8_t nProposalType{0};
    int32_t nHeight{0};

    // payload data
    int32_t nStartHeight{0};
    uint8_t nNumPeriods{0};
    CAmount nAmount{0};
    CScript scriptPayout;
    std::string strName;
    std::string strURL;
    CKeyID ownerKey;

    ADD_SERIALIZE_METHODS;

    template <typename Stream, typename Operation>
    inline void SerializationOp(Stream& s, Operation ser_action)
    {
        READWRITE(nVersion);
        READWRITE(nProposalType);
        READWRITE(nStartHeight);
        READWRITE(nNumPeriods);
        READWRITE(nAmount);
        READWRITE(scriptPayout);
        READWRITE(strName);
        READWRITE(strURL);
        READWRITE(ownerKey);
    }

    std::string ToString() const;

    void ToJson(UniValue& obj) const {
        obj.clear();
        obj.setObject();
        obj.push_back(Pair("version", (int)nVersion));
        obj.push_back(Pair("proposalType", (int)nProposalType));
        obj.push_back(Pair("height", (int)nHeight));
        obj.push_back(Pair("startHeight", (int)nStartHeight));
        obj.push_back(Pair("numPeriods", (int)nNumPeriods));
        obj.push_back(Pair("amount", nAmount));

        CTxDestination dest;
        if (ExtractDestination(scriptPayout, dest)) {
            CBitcoinAddress bitcoinAddress(dest);
            obj.push_back(Pair("payoutAddress", bitcoinAddress.ToString()));
        }
        // obj.push_back(Pair("scriptPayout", scriptPayout));

        obj.push_back(Pair("strName", strName));
        obj.push_back(Pair("strURL", strURL));
        obj.push_back(Pair("ownerAddress", CBitcoinAddress(ownerKey).ToString()));

    }
};

bool CheckProposalTx(const CTransaction& tx, const CBlockIndex* pindexPrev, CValidationState& state);

#endif //DASH_PROPOSALTX_H
