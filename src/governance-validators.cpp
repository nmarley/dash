// Copyright (c) 2014-2017 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "governance-validators.h"

#include "base58.h"
#include "utilstrencodings.h"

#include <algorithm>
#include <sstream>

// TODO: rapidjson for JSONSchema validation : http://rapidjson.org/md_doc_schema.html
#include "rapidjson/document.h"
#include "rapidjson/schema.h"
#include "rapidjson/reader.h"
// #include "rapidjson/writer.h"
// #include "rapidjson/error/en.h"
// #include "rapidjson/stringbuffer.h"
// #include "rapidjson/prettywriter.h"


// #include "valijson/adapters/rapidjson_adapter.hpp"
// #include "valijson/utils/rapidjson_utils.hpp"
// #include "valijson/schema.hpp"
// #include "valijson/schema_parser.hpp"
// #include "valijson/validator.hpp"


CProposalValidator::CProposalValidator(const std::string& strDataHexIn)
    : strDataHex(),
      objJSON(UniValue::VOBJ),
      fJSONValid(false),
      fJSONSchemaValid(false),
      strErrorMessages()
{
    if(!strDataHexIn.empty()) {
        SetHexData(strDataHexIn);
    }
}

void CProposalValidator::Clear()
{
    strDataHex = std::string();
    objJSON = UniValue(UniValue::VOBJ);
    fJSONValid = false;
    fJSONSchemaValid = false;
    strErrorMessages = std::string();
}

void CProposalValidator::SetHexData(const std::string& strDataHexIn)
{
    std::vector<unsigned char> v = ParseHex(strDataHexIn);
    strDataHex = std::string(v.begin(), v.end());
    ParseJSONData();

    // ValidateJSONDataAgainstSchema();
}

bool CProposalValidator::Validate()
{
    if(!ValidateJSON()) {
        strErrorMessages += "JSON parsing error;";
        return false;
    }

    if(!ValidateJSONSchema()) {
        strErrorMessages += "JSON parsing error;";
        return false;
    }

    // TODO: possibly remove all these in lieu of JSONSchema errors
    if(!ValidateName()) {
        strErrorMessages += "Invalid name;";
        return false;
    }
    if(!ValidateStartEndEpoch()) {
        strErrorMessages += "Invalid start:end range;";
        return false;
    }
    if(!ValidatePaymentAmount()) {
        strErrorMessages += "Invalid payment amount;";
        return false;
    }
    if(!ValidatePaymentAddress()) {
        strErrorMessages += "Invalid payment address;";
        return false;
    }
    if(!ValidateURL()) {
        strErrorMessages += "Invalid URL;";
        return false;
    }
    return true;
}

bool CProposalValidator::ValidateJSON()
{
    return fJSONValid;
}

bool CProposalValidator::ValidateJSONSchema()
{
    return fJSONSchemaValid;
}

bool CProposalValidator::ValidateName()
{
    std::string strName;
    if(!GetDataValue("name", strName)) {
        strErrorMessages += "name field not found;";
        return false;
    }

    if(strName.size() > 40) {
        strErrorMessages += "name exceeds 40 characters;";
        return false;
    }

    std::string strNameStripped = StripWhitespace(strName);

    if(strNameStripped.empty()) {
        strErrorMessages += "name is empty;";
        return false;
    }

    static const std::string strAllowedChars = "-_abcdefghijklmnopqrstuvwxyz0123456789";

    std::transform(strName.begin(), strName.end(), strName.begin(), ::tolower);

    if(strName.find_first_not_of(strAllowedChars) != std::string::npos) {
        strErrorMessages += "name contains invalid characters;";
        return false;
    }

    return true;
}

bool CProposalValidator::ValidateStartEndEpoch()
{
    int64_t nStartEpoch = 0;
    int64_t nEndEpoch = 0;

    if(!GetDataValue("start_epoch", nStartEpoch)) {
        strErrorMessages += "start_epoch field not found;";
        return false;
    }

    if(!GetDataValue("end_epoch", nEndEpoch)) {
        strErrorMessages += "end_epoch field not found;";
        return false;
    }

    if(nEndEpoch <= nStartEpoch) {
        strErrorMessages += "end_epoch <= start_epoch;";
        return false;
    }

    return true;
}

bool CProposalValidator::ValidatePaymentAmount()
{
    double dValue = 0.0;

    if(!GetDataValue("payment_amount", dValue)) {
        strErrorMessages += "payment_amount field not found;";
        return false;
    }

    if(dValue <= 0.0) {
        strErrorMessages += "payment_amount is negative;";
        return false;
    }

    // TODO: Should check for an amount which exceeds the budget but this is
    // currently difficult because start and end epochs are defined in terms of
    // clock time instead of block height.

    return true;
}

bool CProposalValidator::ValidatePaymentAddress()
{
    std::string strPaymentAddress;

    if(!GetDataValue("payment_address", strPaymentAddress)) {
        strErrorMessages += "payment_address field not found;";
        return false;
    }

    CBitcoinAddress address(strPaymentAddress);
    if(!address.IsValid()) {
        strErrorMessages += "payment_address is invalid;";
        return false;
    }

    if(address.IsScript()) {
        strErrorMessages += "script addresses are not supported;";
        return false;
    }

    return true;
}

bool CProposalValidator::ValidateURL()
{
    std::string strURL;
    if(!GetDataValue("url", strURL)) {
        strErrorMessages += "url field not found;";
        return false;
    }

    std::string strURLStripped = StripWhitespace(strURL);

    if(strURLStripped.size() < 4U) {
        strErrorMessages += "url too short;";
        return false;
    }

    if(!CheckURL(strURL)) {
        strErrorMessages += "url invalid;";
        return false;
    }

    return true;
}

void CProposalValidator::ParseJSONData()
{
    fJSONValid = false;
    fJSONSchemaValid = false;

    if(strDataHex.empty()) {
        return;
    }

    try {
        UniValue obj(UniValue::VOBJ);
        obj.read(strDataHex);
        std::vector<UniValue> arr1 = obj.getValues();
        std::vector<UniValue> arr2 = arr1.at(0).getValues();
        objJSON = arr2.at(1);
        fJSONValid = true;
    }
    catch(std::exception& e) {
        strErrorMessages += std::string(e.what()) + std::string(";");
    }
    catch(...) {
        strErrorMessages += "Unknown exception;";
    }

    // validate json here...
    rapidjson::Document d;
    d.Parse(PROPOSAL_SCHEMA_V1.c_str());

    // convert this to a SchemaDocument type
    rapidjson::SchemaDocument sd(d);

    rapidjson::SchemaValidator validator(sd);

    rapidjson::StringStream ss(strDataHex.c_str());
    rapidjson::Reader reader;

    reader.Parse(ss, validator);

    // NGM

    // try {
    //     if valid() {
    //         fJSONSchemaValid = true;
    //     }
    // }
    // catch () {
    //     // stuff
    // }

}

bool CProposalValidator::GetDataValue(const std::string& strKey, std::string& strValue)
{
    bool fOK = false;
    try  {
        strValue = objJSON[strKey].get_str();
        fOK = true;
    }
    catch(std::exception& e) {
        strErrorMessages += std::string(e.what()) + std::string(";");
    }
    catch(...) {
        strErrorMessages += "Unknown exception;";
    }
    return fOK;
}

bool CProposalValidator::GetDataValue(const std::string& strKey, int64_t& nValue)
{
    bool fOK = false;
    try  {
        const UniValue uValue = objJSON[strKey];
        switch(uValue.getType()) {
        case UniValue::VNUM:
            nValue = uValue.get_int64();
            fOK = true;
            break;
        default:
            break;
        }
    }
    catch(std::exception& e) {
        strErrorMessages += std::string(e.what()) + std::string(";");
    }
    catch(...) {
        strErrorMessages += "Unknown exception;";
    }
    return fOK;
}

bool CProposalValidator::GetDataValue(const std::string& strKey, double& dValue)
{
    bool fOK = false;
    try  {
        const UniValue uValue = objJSON[strKey];
        switch(uValue.getType()) {
        case UniValue::VNUM:
            dValue = uValue.get_real();
            fOK = true;
            break;
        default:
            break;
        }
    }
    catch(std::exception& e) {
        strErrorMessages += std::string(e.what()) + std::string(";");
    }
    catch(...) {
        strErrorMessages += "Unknown exception;";
    }
    return fOK;
}

std::string CProposalValidator::StripWhitespace(const std::string& strIn)
{
    static const std::string strWhitespace = " \f\n\r\t\v";

    std::string::size_type nStart = strIn.find_first_not_of(strWhitespace);
    std::string::size_type nEnd = strIn.find_last_not_of(strWhitespace);

    if((nStart == std::string::npos) || (nEnd == std::string::npos)) {
        return std::string();
    }

    return strIn.substr(nStart, nEnd - nStart + 1);
}

/*
  The purpose of this function is to replicate the behavior of the
  Python urlparse function used by sentinel (urlparse.py).  This function
  should return false whenever urlparse raises an exception and true
  otherwise.
 */
bool CProposalValidator::CheckURL(const std::string& strURLIn)
{
    std::string strRest(strURLIn);
    std::string::size_type nPos = strRest.find(':');

    if(nPos != std::string::npos) {
        //std::string strSchema = strRest.substr(0,nPos);

        if(nPos < strRest.size()) {
            strRest = strRest.substr(nPos + 1);
        }
        else {
            strRest = "";
        }
    }

    // Process netloc
    if((strRest.size() > 2) && (strRest.substr(0,2) == "//")) {
        static const std::string strNetlocDelimiters = "/?#";

        strRest = strRest.substr(2);

        std::string::size_type nPos2 = strRest.find_first_of(strNetlocDelimiters);

        std::string strNetloc = strRest.substr(0,nPos2);

        if((strNetloc.find('[') != std::string::npos) && (strNetloc.find(']') == std::string::npos)) {
            return false;
        }

        if((strNetloc.find(']') != std::string::npos) && (strNetloc.find('[') == std::string::npos)) {
            return false;
        }
    }

    return true;
}

bool CProposalValidator::GetJSONSchemaForObjectType(const int nObjectType, std::string& strValue)
{
    bool bIsKnownObjectType = false;

    switch (nObjectType) {
        case GOVERNANCE_OBJECT_PROPOSAL:
            bIsKnownObjectType = true;
            strValue = PROPOSAL_SCHEMA_V1;
            break;
        case GOVERNANCE_OBJECT_TRIGGER:
            bIsKnownObjectType = true;
            strValue = TRIGGER_SCHEMA_V1;
            break;
    }

    return bIsKnownObjectType;
}
