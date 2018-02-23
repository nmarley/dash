// Copyright (c) 2014-2017 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#ifndef GOVERNANCE_VALIDATORS_H
#define GOVERNANCE_VALIDATORS_H

#include <string>

#include <univalue.h>

// for governance object type constants
#include "governance-object.h"

static const std::string TRIGGER_SCHEMA_V1 = std::string(R"({
  "$schema": "http://json-schema.org/draft-04/schema#",
  "title": "Trigger",
  "type": "object",
  "properties": {
    "type": {
      "type": "integer",
        "minimum": 2,
        "maximum": 2
    },
    "event_block_height": {
      "type": "integer"
    },
    "payment_addresses": {
      "type": "string"
    },
    "payment_amounts": {
      "type": "string"
    }
  },
  "required": ["type", "event_block_height", "payment_addresses", "payment_amounts"]
})");

class CProposalValidator  {
public:
    CProposalValidator(const std::string& strDataHexIn = std::string());

    void Clear();

    void SetHexData(const std::string& strDataHexIn);

    bool Validate();

    bool ValidateJSON();

    bool ValidateJSONSchema();

    bool ValidateName();

    bool ValidateStartEndEpoch();

    bool ValidatePaymentAmount();

    bool ValidatePaymentAddress();

    bool ValidateURL();

    const std::string& GetErrorMessages()
    {
        return strErrorMessages;
    }

    bool GetJSONSchemaForObjectType(const int nObjectType, std::string& strValue);

private:
    void ParseJSONData();

    bool GetDataValue(const std::string& strKey, std::string& strValue);

    bool GetDataValue(const std::string& strKey, int64_t& nValue);

    bool GetDataValue(const std::string& strKey, double& dValue);

    static std::string StripWhitespace(const std::string& strIn);

    static bool CheckURL(const std::string& strURLIn);


private:
    std::string            strDataHex;

    UniValue               objJSON;

    bool                   fJSONValid;
    bool                   fJSONSchemaValid;

    std::string            strErrorMessages;

};

#endif
